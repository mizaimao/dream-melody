// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
// for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Exact rational durations, measured in whole notes.
//!
//! Never floats. A triplet eighth is `1/12`, and three of them must sum to
//! exactly `1/4` — not to 0.24999999999999997. Every duration in the model is
//! an exact rational so that tuplets, dots, and ties stay closed under
//! addition.

use num_rational::Ratio;

/// Serde support for writing a duration as `"1/8"` instead of a pair of
/// integers.
///
/// Applied with `#[serde(with = "crate::duration::as_string")]`. A saved file is
/// meant to be legible when opened by hand, and `"onset": "3/8"` reads as music
/// where `"onset": [3, 8]` reads as an array of two numbers.
///
/// Parsing reduces (`"2/4"` loads as `1/2`), normalises the sign, and rejects a
/// zero denominator as an error rather than a panic — so a hand-edited file
/// cannot produce a duration the rest of the model would misbehave on.
pub mod as_string {
    use core::str::FromStr;

    use serde::{Deserialize, Deserializer, Serializer};

    use super::Duration;

    /// Writes a duration as a fraction string.
    pub fn serialize<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    /// Reads a fraction string such as `"1/8"`, or a bare integer such as `"2"`.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let text = String::deserialize(deserializer)?;
        Duration::from_str(&text).map_err(serde::de::Error::custom)
    }
}

/// A musical duration in whole notes: a quarter note is `1/4`.
///
/// Also used for onsets, which are measured in whole notes from the start of
/// the track.
pub type Duration = Ratio<i64>;

/// Zero duration — the start of a track, or an empty span.
pub const ZERO: Duration = Ratio::new_raw(0, 1);
/// A whole note.
pub const WHOLE: Duration = Ratio::new_raw(1, 1);
/// A half note.
pub const HALF: Duration = Ratio::new_raw(1, 2);
/// A quarter note.
pub const QUARTER: Duration = Ratio::new_raw(1, 4);
/// An eighth note.
pub const EIGHTH: Duration = Ratio::new_raw(1, 8);
/// A sixteenth note.
pub const SIXTEENTH: Duration = Ratio::new_raw(1, 16);
/// A thirty-second note.
pub const THIRTY_SECOND: Duration = Ratio::new_raw(1, 32);
/// A sixty-fourth note.
pub const SIXTY_FOURTH: Duration = Ratio::new_raw(1, 64);

/// Adds `dots` augmentation dots to a duration.
///
/// Each dot adds half of what came before it, so one dot is 3/2 of the plain
/// value and two dots are 7/4.
pub fn dotted(base: Duration, dots: u32) -> Duration {
    // 2 - 1/2^dots, as an exact rational.
    let scale = 1i64 << dots.min(62);
    base * Ratio::new(2 * scale - 1, scale)
}

/// Scales a duration into a tuplet: `actual` notes played in the time of
/// `normal`.
///
/// A triplet is `actual = 3, normal = 2`. Returns `None` when `actual` is zero,
/// which is not a tuplet.
pub fn tuplet(base: Duration, actual: u32, normal: u32) -> Option<Duration> {
    if actual == 0 {
        return None;
    }
    Some(base * Ratio::new(i64::from(normal), i64::from(actual)))
}

/// Three notes in the time of two.
pub fn triplet(base: Duration) -> Duration {
    base * Ratio::new(2, 3)
}

/// The duration a time signature gives one bar: `4/4` is one whole note.
///
/// Returns `None` if either numeral is zero. A bar of no length is not a bar,
/// and anything that walks a timeline bar by bar would never advance past one.
pub fn bar_length(beats: u32, beat_value: u32) -> Option<Duration> {
    if beats == 0 || beat_value == 0 {
        return None;
    }
    Some(Ratio::new(i64::from(beats), i64::from(beat_value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triplets_sum_exactly() {
        let triplet_eighth = triplet(EIGHTH);
        assert_eq!(triplet_eighth, Ratio::new(1, 12));
        assert_eq!(
            triplet_eighth + triplet_eighth + triplet_eighth,
            QUARTER,
            "three triplet eighths must be exactly a quarter"
        );
    }

    #[test]
    fn dotted_values_are_exact() {
        assert_eq!(dotted(QUARTER, 0), QUARTER);
        assert_eq!(dotted(QUARTER, 1), Ratio::new(3, 8));
        assert_eq!(dotted(QUARTER, 2), Ratio::new(7, 16));
        assert_eq!(dotted(HALF, 1) + QUARTER, WHOLE);
    }

    #[test]
    fn a_dotted_triplet_tied_across_a_bar_stays_exact() {
        // The case that kills floats: a dotted triplet quarter tied to a
        // sixteenth, summed and then decomposed again.
        let dotted_triplet_quarter = dotted(triplet(QUARTER), 1);
        assert_eq!(dotted_triplet_quarter, Ratio::new(1, 4));
        let tied = dotted_triplet_quarter + SIXTEENTH;
        assert_eq!(tied, Ratio::new(5, 16));
        assert_eq!(tied - SIXTEENTH, dotted_triplet_quarter);
    }

    #[test]
    fn tuplet_rejects_a_zero_group() {
        assert_eq!(tuplet(QUARTER, 3, 2), Some(Ratio::new(1, 6)));
        assert_eq!(tuplet(QUARTER, 5, 4), Some(Ratio::new(1, 5)));
        assert_eq!(tuplet(QUARTER, 0, 2), None);
    }

    #[test]
    fn bar_length_matches_common_time_signatures() {
        assert_eq!(bar_length(4, 4), Some(WHOLE));
        assert_eq!(bar_length(3, 4), Some(Ratio::new(3, 4)));
        assert_eq!(bar_length(6, 8), Some(Ratio::new(3, 4)));
        assert_eq!(bar_length(4, 0), None);
        assert_eq!(bar_length(0, 4), None);
    }
}
