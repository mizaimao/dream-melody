// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 Zhenning Zhang
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

//! Tonal pitch classes: position on the line of fifths, with C = 0.
//!
//! ```text
//! ... Fb  Cb  Gb  Db  Ab  Eb  Bb  F   C   G   D   A   E   B   F#  C#  G# ...
//!     -8  -7  -6  -5  -4  -3  -2  -1  0   1   2   3   4   5   6   7   8
//! ```
//!
//! The range is signed and unbounded on purpose. A floor at F-double-flat would
//! bake in a double-accidental ceiling that breaks when transposing C-flat major
//! upward. Clamping belongs in the renderer, where a triple flat genuinely
//! cannot be drawn, not here.

use core::fmt;

/// Semitones above C for each letter, indexed by [`Letter::index`].
const LETTER_SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// The seven note letters, in ascending order from C.
///
/// This is the *spelling* of a pitch, independent of any accidental: the letter
/// determines which staff line or space the notehead sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Letter {
    /// C.
    C,
    /// D.
    D,
    /// E.
    E,
    /// F.
    F,
    /// G.
    G,
    /// A.
    A,
    /// B.
    B,
}

impl Letter {
    /// All seven letters, ascending from C.
    pub const ALL: [Letter; 7] = [
        Letter::C,
        Letter::D,
        Letter::E,
        Letter::F,
        Letter::G,
        Letter::A,
        Letter::B,
    ];

    /// Position in the C-major scale, `C = 0` through `B = 6`.
    pub const fn index(self) -> i32 {
        match self {
            Letter::C => 0,
            Letter::D => 1,
            Letter::E => 2,
            Letter::F => 3,
            Letter::G => 4,
            Letter::A => 5,
            Letter::B => 6,
        }
    }

    /// The letter at `index`, wrapping so that any integer is valid.
    ///
    /// Wrapping rather than returning `Option` keeps callers free of error
    /// handling that could never fire: every integer names a letter once the
    /// octave is accounted for separately.
    pub const fn from_index(index: i32) -> Letter {
        Letter::ALL[index.rem_euclid(7) as usize]
    }

    /// Semitones above the C below, ignoring accidentals: `C = 0`, `B = 11`.
    pub const fn semitone(self) -> i32 {
        LETTER_SEMITONES[self.index() as usize]
    }
}

impl fmt::Display for Letter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Letter::C => "C",
            Letter::D => "D",
            Letter::E => "E",
            Letter::F => "F",
            Letter::G => "G",
            Letter::A => "A",
            Letter::B => "B",
        };
        f.write_str(name)
    }
}

/// A tonal pitch class: a signed position on the line of fifths, with `C = 0`.
///
/// Distinct values may sound alike. `Tpc(8)` is G-sharp and `Tpc(-4)` is
/// A-flat; they share a MIDI number but are not equal, and that distinction is
/// the reason the rest of the model works.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tpc(pub i32);

impl Tpc {
    /// C natural, the origin of the line of fifths.
    pub const C: Tpc = Tpc(0);

    /// The natural (no-accidental) TPC for a letter.
    ///
    /// The naturals occupy `F = -1` through `B = 5`.
    pub const fn natural(letter: Letter) -> Tpc {
        // `letter = mod(4 * tpc, 7)` inverts to `tpc = 2 * letter (mod 7)`,
        // because 4 * 2 = 8 = 1 (mod 7). Shifting into -1..=5 puts F at -1.
        Tpc((2 * letter.index() + 1).rem_euclid(7) - 1)
    }

    /// Build a TPC from a spelling: a letter plus a signed accidental, where
    /// `-1` is a flat, `0` a natural, `+1` a sharp, `-2` a double flat.
    pub const fn new(letter: Letter, accidental: i32) -> Tpc {
        Tpc(Tpc::natural(letter).0 + 7 * accidental)
    }

    /// The letter this TPC is spelled with.
    pub const fn letter(self) -> Letter {
        Letter::from_index(4 * self.0)
    }

    /// The accidental this TPC is spelled with: `-1` flat, `0` natural, `+1`
    /// sharp, and beyond for double accidentals in either direction.
    pub const fn accidental(self) -> i32 {
        (self.0 + 1).div_euclid(7)
    }

    /// Semitones above C, folded into `0..=11`.
    ///
    /// Enharmonic equivalents collapse here: G-sharp and A-flat both give 8.
    /// This is a lossy view, useful only for sounding pitch.
    pub const fn pitch_class(self) -> i32 {
        (self.letter().semitone() + self.accidental()).rem_euclid(12)
    }

    /// How far the spelling reaches beyond a single accidental — `0` for
    /// naturals, flats, and sharps, `1` for double accidentals, and so on.
    ///
    /// Renderers use this to decide what they cannot draw. The model never
    /// clamps.
    pub const fn accidental_depth(self) -> i32 {
        // A signed `saturating_sub` would floor at i32::MIN, not at zero, and
        // report a natural as depth -1.
        let beyond = self.accidental().abs() - 1;
        if beyond < 0 { 0 } else { beyond }
    }
}

impl fmt::Display for Tpc {
    /// Renders ASCII spelling: `C`, `F#`, `Bb`, `Bbb`, `Fx` for F-double-sharp.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.letter())?;
        let accidental = self.accidental();
        match accidental {
            0 => Ok(()),
            n if n < 0 => (0..-n).try_for_each(|_| f.write_str("b")),
            n => {
                // Doubles are conventionally written "x", so F## is "Fx" and a
                // triple sharp is "F#x".
                if n % 2 == 1 {
                    f.write_str("#")?;
                }
                (0..n / 2).try_for_each(|_| f.write_str("x"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table from the module docs, spelled out as the source of truth.
    const LINE_OF_FIFTHS: [(i32, &str); 17] = [
        (-8, "Fb"),
        (-7, "Cb"),
        (-6, "Gb"),
        (-5, "Db"),
        (-4, "Ab"),
        (-3, "Eb"),
        (-2, "Bb"),
        (-1, "F"),
        (0, "C"),
        (1, "G"),
        (2, "D"),
        (3, "A"),
        (4, "E"),
        (5, "B"),
        (6, "F#"),
        (7, "C#"),
        (8, "G#"),
    ];

    #[test]
    fn spellings_match_the_line_of_fifths() {
        for (value, name) in LINE_OF_FIFTHS {
            assert_eq!(Tpc(value).to_string(), name, "tpc {value}");
        }
    }

    #[test]
    fn letter_and_accidental_round_trip_through_new() {
        for value in -60..=60 {
            let tpc = Tpc(value);
            assert_eq!(
                Tpc::new(tpc.letter(), tpc.accidental()),
                tpc,
                "tpc {value} did not survive a spelling round trip"
            );
        }
    }

    #[test]
    fn naturals_span_f_to_b() {
        assert_eq!(Tpc::natural(Letter::F), Tpc(-1));
        assert_eq!(Tpc::natural(Letter::C), Tpc(0));
        assert_eq!(Tpc::natural(Letter::B), Tpc(5));
        for letter in Letter::ALL {
            assert_eq!(Tpc::natural(letter).accidental(), 0);
            assert_eq!(Tpc::natural(letter).letter(), letter);
        }
    }

    #[test]
    fn double_accidentals_are_unbounded_in_both_directions() {
        assert_eq!(Tpc::new(Letter::B, -2).to_string(), "Bbb");
        assert_eq!(Tpc::new(Letter::F, 2).to_string(), "Fx");
        assert_eq!(Tpc::new(Letter::F, 3).to_string(), "F#x");
        assert_eq!(Tpc::new(Letter::C, -3).to_string(), "Cbbb");
        // Depth is what a renderer clamps on; the model just reports it.
        assert_eq!(Tpc::new(Letter::C, 0).accidental_depth(), 0);
        assert_eq!(Tpc::new(Letter::C, -1).accidental_depth(), 0);
        assert_eq!(Tpc::new(Letter::C, -2).accidental_depth(), 1);
        assert_eq!(Tpc::new(Letter::C, 3).accidental_depth(), 2);
    }

    #[test]
    fn enharmonics_share_a_pitch_class_but_not_a_value() {
        let g_sharp = Tpc::new(Letter::G, 1);
        let a_flat = Tpc::new(Letter::A, -1);
        assert_ne!(g_sharp, a_flat);
        assert_eq!(g_sharp.pitch_class(), a_flat.pitch_class());
        // B# wraps to 0, the pitch class of C, without wrapping its letter.
        assert_eq!(Tpc::new(Letter::B, 1).pitch_class(), 0);
        assert_eq!(Tpc::new(Letter::B, 1).letter(), Letter::B);
        assert_eq!(Tpc::new(Letter::C, -1).pitch_class(), 11);
    }
}
