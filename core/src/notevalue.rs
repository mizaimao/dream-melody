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

//! Naming a duration the way notation names one: a note head, some dots, and
//! possibly a tuplet.
//!
//! The model stores duration as an exact rational, which is the right thing to
//! compute with and the wrong thing to draw. Every notation format needs the
//! other view — MusicXML wants `<type>eighth</type><dot/>`, a staff renderer
//! wants a glyph — so the translation lives here once rather than in each.

use num_rational::Ratio;

use crate::duration::Duration;

/// The note values notation has names for, from a whole note down.
const HEADS: [(&str, i64); 8] = [
    ("whole", 1),
    ("half", 2),
    ("quarter", 4),
    ("eighth", 8),
    ("16th", 16),
    ("32nd", 32),
    ("64th", 64),
    ("128th", 128),
];

/// The tuplets worth recognising, as `(actual, normal)`: three in the time of
/// two, five in the time of four, and so on.
const TUPLETS: [(i64, i64); 6] = [(1, 1), (3, 2), (5, 4), (6, 4), (7, 4), (9, 8)];

/// A duration named the way notation names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteValue {
    /// The note head's name: `"quarter"`, `"eighth"`, `"16th"`.
    pub head: &'static str,
    /// Augmentation dots.
    pub dots: u32,
    /// How many of these are played in the time of [`normal`](Self::normal).
    /// `1` when this is not a tuplet.
    pub actual: i64,
    /// How many undotted heads that time would otherwise hold.
    pub normal: i64,
}

impl NoteValue {
    /// Names a duration, if notation has a name for it.
    ///
    /// Returns `None` for a length no head-and-dots-and-tuplet combination
    /// reaches. Callers must then fall back on stating the duration exactly —
    /// approximating it is never the answer.
    pub fn of(dur: Duration) -> Option<NoteValue> {
        for (actual, normal) in TUPLETS {
            for (head, divisor) in HEADS {
                for dots in 0..=3u32 {
                    // A head is 1/divisor of a whole note; each dot adds half
                    // of what came before.
                    let dotted = Ratio::new((1i64 << (dots + 1)) - 1, divisor << dots);
                    if dotted * Ratio::new(normal, actual) == dur {
                        return Some(NoteValue {
                            head,
                            dots,
                            actual,
                            normal,
                        });
                    }
                }
            }
        }
        None
    }

    /// Whether this value is part of a tuplet.
    pub fn is_tuplet(&self) -> bool {
        self.actual != self.normal
    }

    /// The duration this value names.
    pub fn duration(&self) -> Option<Duration> {
        let divisor = HEADS
            .iter()
            .find(|(head, _)| *head == self.head)
            .map(|(_, divisor)| *divisor)?;
        if self.actual == 0 {
            return None;
        }
        let dotted = Ratio::new((1i64 << (self.dots + 1)) - 1, divisor << self.dots);
        Some(dotted * Ratio::new(self.normal, self.actual))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::{EIGHTH, HALF, QUARTER, SIXTEENTH, WHOLE};

    #[test]
    fn plain_values_are_named_plainly() {
        assert_eq!(NoteValue::of(WHOLE).unwrap().head, "whole");
        assert_eq!(NoteValue::of(HALF).unwrap().head, "half");
        assert_eq!(NoteValue::of(QUARTER).unwrap().head, "quarter");
        assert_eq!(NoteValue::of(EIGHTH).unwrap().head, "eighth");
        assert_eq!(NoteValue::of(SIXTEENTH).unwrap().head, "16th");
        assert_eq!(NoteValue::of(QUARTER).unwrap().dots, 0);
        assert!(!NoteValue::of(QUARTER).unwrap().is_tuplet());
    }

    #[test]
    fn dots_are_counted() {
        let dotted_quarter = NoteValue::of(QUARTER + EIGHTH).unwrap();
        assert_eq!((dotted_quarter.head, dotted_quarter.dots), ("quarter", 1));
        let double_dotted = NoteValue::of(HALF + QUARTER + EIGHTH).unwrap();
        assert_eq!((double_dotted.head, double_dotted.dots), ("half", 2));
    }

    #[test]
    fn a_triplet_is_named_as_one() {
        let triplet_eighth = NoteValue::of(Ratio::new(1, 12)).unwrap();
        assert_eq!(triplet_eighth.head, "eighth");
        assert_eq!((triplet_eighth.actual, triplet_eighth.normal), (3, 2));
        assert!(triplet_eighth.is_tuplet());
    }

    #[test]
    fn every_name_reproduces_its_own_duration() {
        let cases = [
            WHOLE,
            HALF,
            QUARTER,
            EIGHTH,
            SIXTEENTH,
            QUARTER + EIGHTH,
            Ratio::new(1, 12),
            Ratio::new(1, 20),
            Ratio::new(1, 6),
        ];
        for dur in cases {
            let value = NoteValue::of(dur).unwrap_or_else(|| panic!("unnamed: {dur}"));
            assert_eq!(value.duration(), Some(dur));
        }
    }

    #[test]
    fn an_unnameable_length_says_so_rather_than_rounding() {
        // Eleven in the time of eight is not a tuplet anyone writes.
        assert_eq!(NoteValue::of(Ratio::new(1, 11)), None);
        assert_eq!(NoteValue::of(Ratio::new(5, 7)), None);
    }
}
