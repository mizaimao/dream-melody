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

use serde::{Deserialize, Serialize};

use crate::interval::Interval;

/// Semitones above C for each letter, indexed by [`Letter::index`].
const LETTER_SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// The seven note letters, in ascending order from C.
///
/// This is the *spelling* of a pitch, independent of any accidental: the letter
/// determines which staff line or space the notehead sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
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

    /// Shifts along the line of fifths. This is the whole of transposition.
    ///
    /// Spelling follows for free: A-flat moved by a major third down is F-flat,
    /// not E, because the shift lands where it lands.
    pub const fn transpose(self, by: Interval) -> Tpc {
        Tpc(self.0 + by.fifths())
    }

    /// How a key signature spells a given letter.
    ///
    /// `key_fifths` is the signed count a key signature is usually quoted by:
    /// `+1` is one sharp, `-4` is four flats. In four flats, E is spelled
    /// E-flat and F is spelled F.
    pub const fn in_key(letter: Letter, key_fifths: i32) -> Tpc {
        // The seven spellings a key uses are the run of fifths from one below
        // the tonic to five above it. Shift this letter's natural into that run.
        let natural = Tpc::natural(letter).0;
        Tpc(natural + 7 * (key_fifths + 5 - natural).div_euclid(7))
    }

    /// The spelling of a sounding pitch class that best fits a key.
    ///
    /// Exactly one spelling of any pitch class lies in any run of twelve
    /// fifths, so this picks the one nearest the key's centre: in four flats,
    /// pitch class 8 is A-flat, and in four sharps it is G-sharp.
    pub const fn nearest_in_key(pitch_class: i32, key_fifths: i32) -> Tpc {
        let mut fifths = key_fifths - 5;
        while fifths <= key_fifths + 6 {
            if Tpc(fifths).pitch_class() == pitch_class.rem_euclid(12) {
                return Tpc(fifths);
            }
            fifths += 1;
        }
        // Unreachable: twelve consecutive fifths cover every pitch class.
        Tpc(key_fifths)
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

/// Why a spelling could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseTpcError {
    /// The text that could not be read.
    pub text: String,
}

impl fmt::Display for ParseTpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a note spelling", self.text)
    }
}

impl std::error::Error for ParseTpcError {}

impl core::str::FromStr for Tpc {
    type Err = ParseTpcError;

    /// Reads what [`Display`](fmt::Display) writes: `C`, `F#`, `Bb`, `Bbb`,
    /// `Fx`, `F#x`.
    fn from_str(text: &str) -> Result<Tpc, ParseTpcError> {
        let fail = || ParseTpcError {
            text: text.to_string(),
        };
        let mut chars = text.chars();
        let letter = match chars.next().ok_or_else(fail)? {
            'C' => Letter::C,
            'D' => Letter::D,
            'E' => Letter::E,
            'F' => Letter::F,
            'G' => Letter::G,
            'A' => Letter::A,
            'B' => Letter::B,
            _ => return Err(fail()),
        };

        let mut accidental = 0;
        for symbol in chars {
            accidental += match symbol {
                'b' => -1,
                '#' => 1,
                'x' => 2,
                _ => return Err(fail()),
            };
        }
        // Flats and sharps in the same breath are a contradiction, not a note.
        if text.contains('b') && (text.contains('#') || text.contains('x')) {
            return Err(fail());
        }
        Ok(Tpc::new(letter, accidental))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

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
    fn every_spelling_reads_back_as_itself() {
        for value in -60..=60 {
            let tpc = Tpc(value);
            assert_eq!(Tpc::from_str(&tpc.to_string()), Ok(tpc), "tpc {value}");
        }
    }

    #[test]
    fn nonsense_spellings_are_refused() {
        for text in ["", "H", "Cb#", "C#b", "Cq", "4", " C", "Cbb "] {
            assert!(Tpc::from_str(text).is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn a_key_signature_spells_its_seven_letters() {
        // Four flats: B, E, A and D are flattened, C, F and G are not.
        let four_flats: Vec<String> = Letter::ALL
            .iter()
            .map(|&letter| Tpc::in_key(letter, -4).to_string())
            .collect();
        assert_eq!(four_flats, ["C", "Db", "Eb", "F", "G", "Ab", "Bb"]);

        let three_sharps: Vec<String> = Letter::ALL
            .iter()
            .map(|&letter| Tpc::in_key(letter, 3).to_string())
            .collect();
        assert_eq!(three_sharps, ["C#", "D", "E", "F#", "G#", "A", "B"]);

        // Seven flats and seven sharps stay single, as they must.
        for key_fifths in -7..=7 {
            for letter in Letter::ALL {
                assert_eq!(Tpc::in_key(letter, key_fifths).accidental_depth(), 0);
            }
        }
        // Eight flats is F-flat major, and it needs one double flat.
        let eight_flats: Vec<String> = Letter::ALL
            .iter()
            .map(|&letter| Tpc::in_key(letter, -8).to_string())
            .collect();
        assert!(eight_flats.contains(&"Bbb".to_string()), "{eight_flats:?}");
    }

    #[test]
    fn a_pitch_class_is_spelled_to_suit_the_key() {
        // The same sound, spelled to match the company it keeps.
        assert_eq!(Tpc::nearest_in_key(8, -4).to_string(), "Ab");
        assert_eq!(Tpc::nearest_in_key(8, 4).to_string(), "G#");
        assert_eq!(Tpc::nearest_in_key(0, 0).to_string(), "C");
        // Every pitch class resolves, in every key, to something that sounds
        // like what was asked for.
        for key_fifths in -10..=10 {
            for pitch_class in 0..12 {
                let tpc = Tpc::nearest_in_key(pitch_class, key_fifths);
                assert_eq!(tpc.pitch_class(), pitch_class);
            }
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
