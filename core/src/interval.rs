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

//! Intervals: the unit of transposition.
//!
//! Transposition takes an interval, never a semitone count, because a semitone
//! count cannot decide spelling. Up one semitone from C is D-flat if you shift
//! `-5` along the line of fifths and C-sharp if you shift `+7`; both sound
//! identical and only one is right for a given key. [`Interval::by_semitones`]
//! is the convenience layer that picks for you, and it is a layer, not the
//! primitive.

use crate::pitch::Tpc;

/// A directed interval — how far to move, both on the staff and in spelling.
///
/// The two components are not independent: `steps` must be congruent to
/// `4 * fifths` modulo 7, because the line of fifths already determines which
/// letter you land on. The remaining freedom is only how many octaves come
/// with it, which is why a major third up and a minor sixth down share a
/// `fifths` and differ in `steps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Interval {
    steps: i32,
    fifths: i32,
}

impl Interval {
    /// No movement at all.
    pub const UNISON: Interval = Interval {
        steps: 0,
        fifths: 0,
    };

    /// One perfect octave up.
    pub const OCTAVE: Interval = Interval {
        steps: 7,
        fifths: 0,
    };

    /// Builds an interval from its staff distance and its line-of-fifths shift.
    ///
    /// Returns `None` when the two disagree — when `steps` is not congruent to
    /// `4 * fifths` modulo 7 — because no such interval exists. A third whose
    /// spelling lands on a letter a fourth away is not a mistake to be
    /// corrected; it is not an interval.
    pub const fn new(steps: i32, fifths: i32) -> Option<Interval> {
        if steps.rem_euclid(7) != (4 * fifths).rem_euclid(7) {
            return None;
        }
        Some(Interval { steps, fifths })
    }

    /// The smallest interval with the given line-of-fifths shift.
    ///
    /// Every `fifths` admits infinitely many intervals, one per octave. This
    /// picks the one that moves the least in sound, which is never more than a
    /// tritone: `+1` is a fourth down rather than a fifth up, and `-4` is a
    /// major third down rather than a minor sixth up. A tritone is a coin flip;
    /// it resolves upward.
    pub const fn simplest(fifths: i32) -> Interval {
        let ascending = (4 * fifths).rem_euclid(7);
        let up = Interval {
            steps: ascending,
            fifths,
        };
        let down = Interval {
            steps: ascending - 7,
            fifths,
        };
        if down.semitones().abs() < up.semitones().abs() {
            down
        } else {
            up
        }
    }

    /// The interval that carries one tonic to another by the shortest route.
    ///
    /// This is what "transpose to B major" means: the shift is fixed by the two
    /// keys, and the octave is chosen so the music moves as little as possible.
    pub const fn between_tonics(from: Tpc, to: Tpc) -> Interval {
        Interval::simplest(to.0 - from.0)
    }

    /// The interval spanning `semitones`, spelled for the key you are leaving.
    ///
    /// Two spellings always fit a given semitone count — one sharpward, one
    /// flatward. This picks whichever gives the destination key signature fewer
    /// accidentals, so C major up a semitone becomes D-flat major with five
    /// flats rather than C-sharp major with seven sharps. A tie keeps the
    /// sharpward reading, which is why the tritone above C is F-sharp.
    pub const fn by_semitones(semitones: i32, from_tonic: Tpc) -> Interval {
        // Seven fifths make a semitone, and 7 * 7 = 1 (mod 12), so the shift
        // that moves `semitones` is 7 * semitones (mod 12).
        let sharpward = (7 * semitones).rem_euclid(12);
        let flatward = sharpward - 12;
        let sharp_key = (from_tonic.0 + sharpward).abs();
        let flat_key = (from_tonic.0 + flatward).abs();
        let fifths = if flat_key < sharp_key {
            flatward
        } else {
            sharpward
        };

        // `simplest` may land an octave off the requested displacement, since
        // it only knows the shift. Correct back onto the octave asked for.
        let base = Interval::simplest(fifths);
        let octaves = (semitones - base.semitones()).div_euclid(12);
        base.with_octaves(octaves)
    }

    /// Moves the interval by whole octaves, keeping its spelling.
    pub const fn with_octaves(self, octaves: i32) -> Interval {
        Interval {
            steps: self.steps + 7 * octaves,
            fifths: self.fifths,
        }
    }

    /// The same interval in the opposite direction.
    pub const fn inverted(self) -> Interval {
        Interval {
            steps: -self.steps,
            fifths: -self.fifths,
        }
    }

    /// Distance on the staff: `0` is a unison, `1` a second, `7` an octave.
    /// Negative descends.
    pub const fn steps(self) -> i32 {
        self.steps
    }

    /// Distance along the line of fifths — the component that decides spelling.
    pub const fn fifths(self) -> i32 {
        self.fifths
    }

    /// How far this interval moves in sound. Negative descends.
    pub const fn semitones(self) -> i32 {
        // Apply the interval to C0 and read off where it lands.
        let landed = Tpc(self.fifths);
        let octaves = (self.steps - landed.letter().index()).div_euclid(7);
        12 * octaves + landed.letter().semitone() + landed.accidental()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::Letter;

    #[test]
    fn named_intervals_have_the_semitones_they_should() {
        let cases = [
            (Interval::UNISON, 0),
            (Interval::OCTAVE, 12),
            // Minor second up: C to D-flat, five fifths flatward.
            (Interval::new(1, -5).unwrap(), 1),
            // Augmented unison: C to C-sharp, seven fifths sharpward. Same
            // sound as above, different interval entirely.
            (Interval::new(0, 7).unwrap(), 1),
            (Interval::new(1, 2).unwrap(), 2),    // major second
            (Interval::new(2, -3).unwrap(), 3),   // minor third
            (Interval::new(2, 4).unwrap(), 4),    // major third
            (Interval::new(3, -1).unwrap(), 5),   // perfect fourth
            (Interval::new(4, 1).unwrap(), 7),    // perfect fifth
            (Interval::new(-4, -1).unwrap(), -7), // perfect fifth down
            (Interval::new(-2, -4).unwrap(), -4), // major third down
        ];
        for (interval, semitones) in cases {
            assert_eq!(interval.semitones(), semitones, "{interval:?}");
        }
    }

    #[test]
    fn steps_and_fifths_must_agree() {
        // A second must be two fifths sharpward or five flatward, never one.
        assert!(Interval::new(1, 1).is_none());
        assert!(Interval::new(1, 2).is_some());
        assert!(Interval::new(1, -5).is_some());
        // Every fifths value admits exactly one staff distance per octave.
        for fifths in -20..=20 {
            let valid = (0..7).filter(|&s| Interval::new(s, fifths).is_some());
            assert_eq!(valid.count(), 1, "fifths {fifths}");
        }
    }

    #[test]
    fn simplest_takes_the_shorter_road() {
        // A fourth down, not a fifth up: five semitones beats seven.
        assert_eq!(Interval::simplest(1).semitones(), -5);
        // Nothing ever moves more than a tritone.
        for fifths in -40..=40 {
            assert!(Interval::simplest(fifths).semitones().abs() <= 6);
        }
        // A major third down, not a minor sixth up.
        assert_eq!(Interval::simplest(-4).semitones(), -4);
        // An augmented second up, not a diminished seventh down.
        assert_eq!(Interval::simplest(9).semitones(), 3);
        // The tritone resolves upward on a tie.
        assert_eq!(Interval::simplest(6).semitones(), 6);
        assert_eq!(Interval::simplest(0), Interval::UNISON);
    }

    #[test]
    fn inversion_round_trips() {
        for fifths in -20..=20 {
            let interval = Interval::simplest(fifths);
            assert_eq!(interval.inverted().inverted(), interval);
            assert_eq!(interval.inverted().semitones(), -interval.semitones());
        }
    }

    /// Invariant 2. Up a minor second from C major is D-flat, not C-sharp.
    #[test]
    fn by_semitones_prefers_the_key_with_fewer_accidentals() {
        let c_major = Tpc::C;
        let up_one = Interval::by_semitones(1, c_major);
        assert_eq!(up_one.semitones(), 1);
        // D-flat: five flats. C-sharp would be seven sharps.
        assert_eq!(Tpc(c_major.0 + up_one.fifths()).to_string(), "Db");

        // From D major, up one semitone is E-flat (3 flats), not D-sharp (9).
        let d_major = Tpc::new(Letter::D, 0);
        let from_d = Interval::by_semitones(1, d_major);
        assert_eq!(Tpc(d_major.0 + from_d.fifths()).to_string(), "Eb");

        // But from B major, up one is C major — no accidentals at all — rather
        // than B-sharp major, which is absurd.
        let b_major = Tpc::new(Letter::B, 0);
        let from_b = Interval::by_semitones(1, b_major);
        assert_eq!(Tpc(b_major.0 + from_b.fifths()).to_string(), "C");
    }

    #[test]
    fn by_semitones_lands_on_the_requested_displacement() {
        for semitones in -24..=24 {
            for tonic in -7..=7 {
                let interval = Interval::by_semitones(semitones, Tpc(tonic));
                assert_eq!(
                    interval.semitones(),
                    semitones,
                    "{semitones} semitones from tpc {tonic}"
                );
            }
        }
    }
}
