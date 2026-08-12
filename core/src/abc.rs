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

//! ABC notation, written out.
//!
//! ABC is the format people paste into forum posts and feed to session
//! tune-books, so it is worth exporting even though MusicXML is the serious
//! interchange. Export only: reading ABC means implementing a fairly baroque
//! grammar for little gain, and MusicXML already covers coming back in.
//!
//! Accidentals appear only where a note differs from the key signature, which
//! is what makes ABC readable. Everything else follows from `K:`.

use core::fmt::Write as _;

use crate::duration::Duration;
use crate::model::{Document, Mode, Track};
use crate::pitch::Tpc;

/// Why a document could not be written as ABC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbcError {
    /// The time signature has no bar length.
    InvalidTimeSignature(crate::model::TimeSignature),
}

impl core::fmt::Display for AbcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AbcError::InvalidTimeSignature(time_sig) => {
                write!(f, "cannot draw bars in time signature {time_sig}")
            }
        }
    }
}

impl std::error::Error for AbcError {}

/// The `K:` suffix each mode takes.
fn mode_suffix(mode: Mode) -> &'static str {
    match mode {
        Mode::Major => "",
        Mode::Minor => "m",
        Mode::Dorian => "Dor",
        Mode::Phrygian => "Phr",
        Mode::Lydian => "Lyd",
        Mode::Mixolydian => "Mix",
        Mode::Locrian => "Loc",
    }
}

/// The unit note length: the largest half-power that divides evenly into the
/// shortest note, so ordinary music needs no fractions.
fn unit_length(track: &Track, bar: Duration) -> Duration {
    let shortest = track
        .segments(bar)
        .iter()
        .map(|segment| segment.event.dur())
        .min()
        .unwrap_or(crate::duration::EIGHTH);

    let mut unit = Duration::new(1, 1);
    for _ in 0..6 {
        if unit <= shortest {
            break;
        }
        unit /= 2;
    }
    unit
}

/// Writes a pitch: accidental if it differs from the key, letter, octave marks.
fn write_pitch(out: &mut String, tpc: Tpc, octave: i32, key_fifths: i32) {
    if Tpc::in_key(tpc.letter(), key_fifths) != tpc {
        let accidental = tpc.accidental();
        match accidental {
            0 => out.push('='),
            n if n > 0 => out.push_str(&"^".repeat(n as usize)),
            n => out.push_str(&"_".repeat((-n) as usize)),
        }
    }

    // ABC's middle C is a capital C; c is the octave above.
    if octave >= 5 {
        out.push(tpc.letter().to_string().to_ascii_lowercase().remove(0));
        out.push_str(&"'".repeat((octave - 5) as usize));
    } else {
        let _ = write!(out, "{}", tpc.letter());
        out.push_str(&",".repeat((4 - octave) as usize));
    }
}

/// Writes a length as a multiple of the unit note length.
fn write_length(out: &mut String, dur: Duration, unit: Duration) {
    let multiple = dur / unit;
    if multiple == Duration::new(1, 1) {
        return;
    }
    if multiple.is_integer() {
        let _ = write!(out, "{}", multiple.to_integer());
    } else {
        let _ = write!(out, "{}/{}", multiple.numer(), multiple.denom());
    }
}

/// Writes a document as ABC. One tune per track, numbered from 1.
pub fn to_string(document: &Document) -> Result<String, AbcError> {
    let bar = document
        .time_sig
        .bar_length()
        .ok_or(AbcError::InvalidTimeSignature(document.time_sig))?;

    let mut out = String::new();
    for (index, track) in document.tracks.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let unit = unit_length(track, bar);
        let key_fifths = track.key_fifths();

        let _ = writeln!(out, "X:{}", index + 1);
        if !document.title.is_empty() {
            let _ = writeln!(out, "T:{}", document.title);
        }
        let _ = writeln!(out, "M:{}", document.time_sig);
        let _ = writeln!(out, "L:{}/{}", unit.numer(), unit.denom());
        let _ = writeln!(out, "Q:1/4={}", document.bpm);
        let _ = writeln!(out, "K:{}{}", track.tonic_tpc, mode_suffix(track.mode));

        let segments = track.segments(bar);
        let bars = segments.last().map_or(0, |segment| segment.bar + 1);
        let mut body = String::new();
        for measure in 0..bars {
            for segment in segments.iter().filter(|segment| segment.bar == measure) {
                if !body.is_empty() {
                    body.push(' ');
                }
                match segment.event.as_note() {
                    Some(note) => write_pitch(&mut body, note.tpc, note.octave, key_fifths),
                    None => body.push('z'),
                }
                write_length(&mut body, segment.event.dur(), unit);
                // A tie is a hyphen after the note it starts on.
                if segment.tied_to_next {
                    body.push('-');
                }
            }
            if measure + 1 < bars {
                body.push_str(" |");
            }
        }
        let _ = writeln!(out, "{body} |]");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::{QUARTER, ZERO};
    use crate::model::{Note, TimeSignature, Track};
    use crate::pitch::Letter;

    #[test]
    fn octaves_use_case_and_marks() {
        let mut out = String::new();
        write_pitch(&mut out, Tpc::C, 3, 0);
        assert_eq!(out, "C,");
        out.clear();
        write_pitch(&mut out, Tpc::C, 4, 0);
        assert_eq!(out, "C");
        out.clear();
        write_pitch(&mut out, Tpc::C, 5, 0);
        assert_eq!(out, "c");
        out.clear();
        write_pitch(&mut out, Tpc::C, 6, 0);
        assert_eq!(out, "c'");
    }

    #[test]
    fn accidentals_appear_only_against_the_key() {
        let mut out = String::new();
        // E-flat in four flats needs no sign; the key already says so.
        write_pitch(&mut out, Tpc::new(Letter::E, -1), 4, -4);
        assert_eq!(out, "E");
        out.clear();
        // E natural in four flats needs one.
        write_pitch(&mut out, Tpc::new(Letter::E, 0), 4, -4);
        assert_eq!(out, "=E");
        out.clear();
        write_pitch(&mut out, Tpc::new(Letter::F, 1), 4, 0);
        assert_eq!(out, "^F");
        out.clear();
        write_pitch(&mut out, Tpc::new(Letter::B, -2), 4, 0);
        assert_eq!(out, "__B");
    }

    #[test]
    fn modes_take_their_suffix() {
        let document = Document::new(
            "T",
            TimeSignature::COMMON,
            60,
            [Track::new(
                "Melody",
                Tpc::new(Letter::D, 0),
                Mode::Dorian,
                [Note::new(Tpc::new(Letter::D, 0), 4, ZERO, QUARTER * 4).into()],
            )],
        );
        assert!(to_string(&document).unwrap().contains("K:DDor"));
    }
}
