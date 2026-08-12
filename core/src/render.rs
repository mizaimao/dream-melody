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

//! What a renderer needs, worked out once.
//!
//! Drawing a score needs answers this model already has: where the barlines
//! fall, which notes are tied across them, what to call a duration, and what
//! scale degree a note is. Working those out in the renderer would mean
//! working them out twice — once in the staff renderer and once in the
//! numbered-notation one — and the two would drift.
//!
//! So they are worked out here, and both renderers consume the result. It
//! carries every view of a note at once: a step, an alter and an octave for a
//! staff, a digit and octave marks for numbered notation, and a MIDI number
//! for playback. A renderer picks what it needs and draws.

use serde::{Deserialize, Serialize};

use crate::duration::Duration;
use crate::jianpu::{Degree, DurationMarks};
use crate::model::{Document, TimeSignature};
use crate::notevalue::NoteValue;
use crate::pitch::Tpc;

/// Why a score could not be prepared for drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The time signature has no bar length, so there are no bars to draw.
    InvalidTimeSignature(TimeSignature),
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RenderError::InvalidTimeSignature(time_sig) => {
                write!(f, "cannot draw bars in time signature {time_sig}")
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// A score, laid out in bars and ready to draw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderScore {
    /// Title, as displayed.
    pub title: String,
    /// Tempo in quarter notes per minute.
    pub bpm: u32,
    /// Beats per bar — the upper numeral.
    pub beats: u32,
    /// The note value that gets one beat — the lower numeral.
    pub beat_value: u32,
    /// One bar's length in whole notes.
    #[serde(with = "crate::duration::as_string")]
    pub bar_length: Duration,
    /// The voices.
    pub tracks: Vec<RenderTrack>,
}

/// One voice, in bars.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderTrack {
    /// Display name.
    pub name: String,
    /// The key signature, signed: `-4` is four flats.
    pub key_fifths: i32,
    /// The major key of that signature, spelled — `"Ab"` for four flats.
    ///
    /// Staff renderers name a key signature this way rather than by count, and
    /// it is the *signature* they want, so a D dorian part says `"C"`.
    pub key_name: String,
    /// The tonic scale degree 1 sits on, spelled.
    pub tonic: String,
    /// The mode, by name.
    pub mode: String,
    /// The octave an unmarked scale degree sits in.
    pub register: i32,
    /// The bars, in order.
    pub bars: Vec<RenderBar>,
}

/// One bar of one voice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderBar {
    /// Which bar this is, counting from zero.
    pub index: i64,
    /// What sounds in it, in order.
    pub notes: Vec<RenderNote>,
}

/// One thing to draw: a note or a rest, clipped to its bar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderNote {
    /// Whether this is silence.
    pub rest: bool,
    /// Whole notes from the start of the track.
    #[serde(with = "crate::duration::as_string")]
    pub onset: Duration,
    /// Length in whole notes, clipped to this bar.
    #[serde(with = "crate::duration::as_string")]
    pub dur: Duration,

    /// The letter, as a staff wants it: `"E"`. Empty for a rest.
    pub step: String,
    /// The accidental, as a staff wants it: `-1` is a flat.
    pub alter: i32,
    /// Scientific octave, following the letter.
    pub octave: i32,
    /// The spelling, for a label: `"Eb"`. Empty for a rest.
    pub spelling: String,
    /// The MIDI note number, for playback. `-1` for a rest.
    pub midi: i32,

    /// What notation calls this length: `"eighth"`. Empty if notation has no
    /// name for it, in which case a renderer must fall back on the duration.
    pub head: String,
    /// Augmentation dots.
    pub dots: u32,
    /// How many notes in the time of how many, when this is a tuplet.
    pub tuplet: Option<[i64; 2]>,

    /// This continues a note that began in an earlier bar.
    pub tied_from_previous: bool,
    /// This note runs on past the barline.
    pub tied_to_next: bool,

    /// Beams under the digit, in numbered notation: one halves the beat.
    pub beams: u32,
    /// Whole beats to draw as dashes after the digit.
    pub dashes: u32,
    /// The length written as a fraction, when marks cannot say it exactly.
    pub exact: Option<String>,

    /// The numbered-notation digit, `1` to `7`. `0` for a rest.
    pub degree: u32,
    /// The digit's accidental: `-1` flat, `+1` sharp.
    pub degree_alteration: i32,
    /// Octaves above the register the track names. Negative is below.
    pub degree_octave: i32,
}

/// Works out everything a renderer needs from a document.
pub fn of(document: &Document) -> Result<RenderScore, RenderError> {
    let bar = document
        .time_sig
        .bar_length()
        .ok_or(RenderError::InvalidTimeSignature(document.time_sig))?;

    let beat = bar / i64::from(document.time_sig.beats.max(1));

    let mut tracks = Vec::new();
    for track in &document.tracks {
        let key_fifths = track.key_fifths();
        let position = track.register_position();

        let mut bars: Vec<RenderBar> = Vec::new();
        for segment in track.segments(bar) {
            let note = segment.event.as_note();
            let value = NoteValue::of(segment.event.dur());
            let degree = note.map(|note| Degree::of(note, track.tonic_tpc, position));
            let marks = DurationMarks::of(segment.event.dur(), beat);

            let drawn = RenderNote {
                rest: note.is_none(),
                onset: segment.event.onset(),
                dur: segment.event.dur(),

                step: note
                    .map(|note| note.tpc.letter().to_string())
                    .unwrap_or_default(),
                alter: note.map_or(0, |note| note.tpc.accidental()),
                octave: note.map_or(0, |note| note.octave),
                spelling: note.map(|note| note.tpc.to_string()).unwrap_or_default(),
                midi: note.map_or(-1, |note| note.midi()),

                head: value
                    .map(|value| value.head.to_string())
                    .unwrap_or_default(),
                dots: value.map_or(0, |value| value.dots),
                tuplet: value
                    .filter(NoteValue::is_tuplet)
                    .map(|value| [value.actual, value.normal]),

                tied_from_previous: segment.tied_to_previous,
                tied_to_next: segment.tied_to_next,

                beams: marks.beams,
                dashes: marks.dashes,
                exact: marks
                    .exact
                    .map(|value| format!("{}/{}", value.numer(), value.denom())),

                degree: degree.map_or(0, |degree| degree.number),
                degree_alteration: degree.map_or(0, |degree| degree.alteration),
                degree_octave: degree.map_or(0, |degree| degree.octave),
            };

            match bars.last_mut() {
                Some(last) if last.index == segment.bar => last.notes.push(drawn),
                _ => bars.push(RenderBar {
                    index: segment.bar,
                    notes: vec![drawn],
                }),
            }
        }

        tracks.push(RenderTrack {
            name: track.name.clone(),
            key_fifths,
            // For a major key the tonic and the signature are the same point on
            // the line of fifths, so this names the signature in every mode.
            key_name: Tpc(key_fifths).to_string(),
            tonic: track.tonic_tpc.to_string(),
            mode: track.mode.name().to_string(),
            register: track.register,
            bars,
        });
    }

    Ok(RenderScore {
        title: document.title.clone(),
        bpm: document.bpm,
        beats: document.time_sig.beats,
        beat_value: document.time_sig.beat_value,
        bar_length: bar,
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::{EIGHTH, QUARTER, ZERO};
    use crate::model::{Mode, Note, Rest, Track};
    use crate::pitch::Letter;

    fn sample() -> Document {
        Document::new(
            "Sample",
            TimeSignature::COMMON,
            67,
            [Track::new(
                "Melody",
                Tpc::new(Letter::A, -1),
                Mode::Major,
                [
                    Rest::new(ZERO, EIGHTH).into(),
                    Note::new(Tpc::new(Letter::E, -1), 4, EIGHTH, EIGHTH).into(),
                    // Starts on beat 2 and runs into the next bar.
                    Note::new(Tpc::new(Letter::A, -1), 4, QUARTER, QUARTER * 4).into(),
                    Note::new(Tpc::new(Letter::B, -1), 4, QUARTER * 5, QUARTER * 3).into(),
                ],
            )],
        )
    }

    #[test]
    fn a_note_carries_every_view_of_itself_at_once() {
        let score = of(&sample()).unwrap();
        let note = &score.tracks[0].bars[0].notes[1];

        // The staff's view.
        assert_eq!(note.step, "E");
        assert_eq!(note.alter, -1);
        assert_eq!(note.octave, 4);
        assert_eq!(note.spelling, "Eb");
        // Playback's view.
        assert_eq!(note.midi, 63);
        // Numbered notation's view: E-flat is degree 5 in A-flat.
        assert_eq!(note.degree, 5);
        assert_eq!(note.degree_alteration, 0);
        assert_eq!(note.degree_octave, 0);
        // And what to call the length, in both notations: an eighth note on a
        // staff, one beam under the digit in numbered notation.
        assert_eq!(note.head, "eighth");
        assert_eq!(note.dots, 0);
        assert_eq!(note.tuplet, None);
        assert_eq!(note.beams, 1);
        assert_eq!(note.dashes, 0);
        assert_eq!(note.exact, None);
    }

    #[test]
    fn the_key_signature_is_named_the_way_a_staff_wants_it() {
        let score = of(&sample()).unwrap();
        assert_eq!(score.tracks[0].key_fifths, -4);
        assert_eq!(score.tracks[0].key_name, "Ab");
        assert_eq!(score.tracks[0].tonic, "Ab");

        // A dorian part carries the signature of C major, not of D major.
        let dorian = Document::new(
            "Dorian",
            TimeSignature::COMMON,
            60,
            [Track::new(
                "Melody",
                Tpc::new(Letter::D, 0),
                Mode::Dorian,
                [Note::new(Tpc::new(Letter::D, 0), 4, ZERO, QUARTER * 4).into()],
            )],
        );
        let score = of(&dorian).unwrap();
        assert_eq!(score.tracks[0].key_name, "C");
        assert_eq!(score.tracks[0].tonic, "D");
        assert_eq!(score.tracks[0].mode, "Dorian");
    }

    #[test]
    fn a_note_across_a_barline_is_split_and_flagged_for_a_tie() {
        let score = of(&sample()).unwrap();
        let first = &score.tracks[0].bars[0];
        let second = &score.tracks[0].bars[1];

        let opening = first.notes.last().unwrap();
        assert!(opening.tied_to_next, "should run past the barline");
        assert!(!opening.tied_from_previous);
        assert_eq!(opening.dur, QUARTER * 3);

        let continued = &second.notes[0];
        assert!(continued.tied_from_previous, "should continue the note");
        assert!(!continued.tied_to_next);
        assert_eq!(continued.dur, QUARTER);
        // Same pitch on both sides, which is what makes it a tie at all.
        assert_eq!(continued.spelling, opening.spelling);
        assert_eq!(continued.octave, opening.octave);
    }

    #[test]
    fn bars_come_out_numbered_and_in_order() {
        let score = of(&sample()).unwrap();
        assert_eq!(score.title, "Sample");
        assert_eq!(score.bpm, 67);
        assert_eq!((score.beats, score.beat_value), (4, 4));
        assert_eq!(score.bar_length, QUARTER * 4);

        let indices: Vec<i64> = score.tracks[0].bars.iter().map(|bar| bar.index).collect();
        assert_eq!(indices, [0, 1]);
        for bar in &score.tracks[0].bars {
            let total: Duration = bar.notes.iter().map(|note| note.dur).sum();
            assert_eq!(total, score.bar_length, "bar {} is not full", bar.index);
        }
    }

    #[test]
    fn a_rest_says_it_is_one_rather_than_pretending_to_be_a_pitch() {
        let score = of(&sample()).unwrap();
        let rest = &score.tracks[0].bars[0].notes[0];
        assert!(rest.rest);
        assert_eq!(rest.midi, -1);
        assert_eq!(rest.degree, 0);
        assert_eq!(rest.step, "");
        assert_eq!(rest.spelling, "");
        assert_eq!(rest.head, "eighth");
    }

    #[test]
    fn the_marks_a_digit_takes_match_the_note_it_is() {
        let document = Document::new(
            "Lengths",
            TimeSignature::COMMON,
            60,
            [Track::new(
                "Melody",
                Tpc::C,
                Mode::Major,
                [
                    // A whole note: one digit and three dashes.
                    Note::new(Tpc::C, 4, ZERO, QUARTER * 4).into(),
                    // A dotted quarter, then an eighth to fill the beat.
                    Note::new(Tpc::C, 4, QUARTER * 4, QUARTER + EIGHTH).into(),
                    Note::new(Tpc::C, 4, QUARTER * 4 + QUARTER + EIGHTH, EIGHTH).into(),
                    // Two sixteenths and a half to finish the bar.
                    Note::new(Tpc::C, 4, QUARTER * 6, EIGHTH / 2).into(),
                    Note::new(Tpc::C, 4, QUARTER * 6 + EIGHTH / 2, EIGHTH / 2).into(),
                    Note::new(Tpc::C, 4, QUARTER * 6 + EIGHTH, QUARTER + EIGHTH).into(),
                ],
            )],
        );
        let score = of(&document).unwrap();
        let marks: Vec<(u32, u32, u32)> = score.tracks[0]
            .bars
            .iter()
            .flat_map(|bar| &bar.notes)
            .map(|note| (note.beams, note.dots, note.dashes))
            .collect();
        assert_eq!(
            marks,
            [
                (0, 0, 3), // whole: three dashes
                (0, 1, 0), // dotted quarter: one dot
                (1, 0, 0), // eighth: one beam
                (2, 0, 0), // sixteenth: two beams
                (2, 0, 0),
                (0, 1, 0), // dotted quarter again
            ]
        );
        assert!(
            score.tracks[0]
                .bars
                .iter()
                .flat_map(|bar| &bar.notes)
                .all(|note| note.exact.is_none())
        );
    }

    #[test]
    fn a_length_no_marks_can_say_is_given_as_a_fraction() {
        let third = QUARTER / 3;
        let document = Document::new(
            "Triplet",
            TimeSignature {
                beats: 1,
                beat_value: 4,
            },
            60,
            [Track::new(
                "Melody",
                Tpc::C,
                Mode::Major,
                [
                    Note::new(Tpc::C, 4, ZERO, third).into(),
                    Note::new(Tpc::C, 4, third, third).into(),
                    Note::new(Tpc::C, 4, third * 2, third).into(),
                ],
            )],
        );
        let score = of(&document).unwrap();
        let note = &score.tracks[0].bars[0].notes[0];
        assert_eq!(note.exact.as_deref(), Some("1/12"));
        assert_eq!((note.beams, note.dots, note.dashes), (0, 0, 0));
    }

    #[test]
    fn a_triplet_says_what_it_is_in_the_time_of() {
        let third = QUARTER / 3;
        let document = Document::new(
            "Triplet",
            TimeSignature {
                beats: 1,
                beat_value: 4,
            },
            60,
            [Track::new(
                "Melody",
                Tpc::C,
                Mode::Major,
                [
                    Note::new(Tpc::C, 4, ZERO, third).into(),
                    Note::new(Tpc::C, 4, third, third).into(),
                    Note::new(Tpc::C, 4, third * 2, third).into(),
                ],
            )],
        );
        let score = of(&document).unwrap();
        let note = &score.tracks[0].bars[0].notes[0];
        assert_eq!(note.head, "eighth");
        assert_eq!(note.tuplet, Some([3, 2]));
    }
}
