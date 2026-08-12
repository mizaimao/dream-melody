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

//! The document model: notes, rests, tracks, and the timeline rules they obey.
//!
//! Ties are not stored. A tie joins two noteheads of the same pitch performed
//! as one sustained sound, and exists only because notation cannot draw a note
//! crossing a barline. Two tied noteheads are therefore one [`Note`] with the
//! summed duration, and the renderer splits it again when drawing. "How long is
//! this note" always has a direct answer.
//!
//! Slurs are not ties and are not modelled at all.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::duration::{self, Duration};
use crate::interval::Interval;
use crate::pitch::Tpc;

/// A time signature, as written: `4/4` is `beats = 4, beat_value = 4`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSignature {
    /// Beats per bar — the upper numeral.
    pub beats: u32,
    /// The note value that gets one beat — the lower numeral.
    pub beat_value: u32,
}

impl TimeSignature {
    /// Common time, `4/4`.
    pub const COMMON: TimeSignature = TimeSignature {
        beats: 4,
        beat_value: 4,
    };

    /// How much music fills one bar, in whole notes.
    ///
    /// Returns `None` for a zero `beat_value`, which is not a time signature.
    pub fn bar_length(self) -> Option<Duration> {
        duration::bar_length(self.beats, self.beat_value)
    }
}

impl fmt::Display for TimeSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.beats, self.beat_value)
    }
}

/// The mode a track's tonic is read in.
///
/// This selects which scale degree the tonic sits on, which is what a numbered
/// notation (jianpu/简谱) reader needs; it does not change how pitch is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Mode {
    /// Ionian.
    #[default]
    Major,
    /// Aeolian — natural minor.
    Minor,
    /// Dorian.
    Dorian,
    /// Phrygian.
    Phrygian,
    /// Lydian.
    Lydian,
    /// Mixolydian.
    Mixolydian,
    /// Locrian.
    Locrian,
}

impl Mode {
    /// Every mode, in the order their tonics sit on the line of fifths.
    pub const ALL: [Mode; 7] = [
        Mode::Lydian,
        Mode::Major,
        Mode::Mixolydian,
        Mode::Dorian,
        Mode::Minor,
        Mode::Phrygian,
        Mode::Locrian,
    ];

    /// The name used in text formats and in the UI.
    pub const fn name(self) -> &'static str {
        match self {
            Mode::Major => "Major",
            Mode::Minor => "Minor",
            Mode::Dorian => "Dorian",
            Mode::Phrygian => "Phrygian",
            Mode::Lydian => "Lydian",
            Mode::Mixolydian => "Mixolydian",
            Mode::Locrian => "Locrian",
        }
    }

    /// Reads a mode by name, case-insensitively.
    pub fn from_name(name: &str) -> Option<Mode> {
        Mode::ALL
            .into_iter()
            .find(|mode| mode.name().eq_ignore_ascii_case(name))
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A sounding note.
///
/// `octave` is scientific and **letter-based, not pitch-based**: B-sharp 4 is
/// octave 4 and sounds as MIDI 72, because the letter B belongs to octave 4
/// regardless of the sharp pushing it above C5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Note {
    /// Line-of-fifths position, C = 0.
    pub tpc: Tpc,
    /// Scientific octave, where C4 is middle C. Follows the letter.
    pub octave: i32,
    /// Whole notes from the start of the track.
    #[serde(with = "crate::duration::as_string")]
    pub onset: Duration,
    /// Length in whole notes, with any tied values already summed.
    #[serde(with = "crate::duration::as_string")]
    pub dur: Duration,
}

impl Note {
    /// Builds a note.
    pub fn new(tpc: Tpc, octave: i32, onset: Duration, dur: Duration) -> Note {
        Note {
            tpc,
            octave,
            onset,
            dur,
        }
    }

    /// The MIDI note number, where C4 (middle C) is 60.
    ///
    /// Derived from the letter's octave, not from a folded pitch class, so
    /// B-sharp 4 gives 72 and C-flat 4 gives 59.
    pub fn midi(&self) -> i32 {
        (self.octave + 1) * 12 + self.tpc.letter().semitone() + self.tpc.accidental()
    }

    /// Where the note stops sounding, in whole notes from the start.
    pub fn end(&self) -> Duration {
        self.onset + self.dur
    }

    /// Absolute position on the staff, counting letters: C0 is 0, D0 is 1.
    ///
    /// Accidentals do not move a notehead, so this ignores them. It is what a
    /// renderer needs to place the note, and what transposition moves.
    pub fn diatonic_position(&self) -> i32 {
        self.octave * 7 + self.tpc.letter().index()
    }

    /// Moves the note by an interval, keeping its timing.
    ///
    /// The octave is derived rather than guessed: the interval says how far to
    /// move on the staff, the shift says what the new spelling is, and those
    /// two agree by construction, so the octave falls out exactly.
    pub fn transpose(&self, by: Interval) -> Note {
        let tpc = self.tpc.transpose(by);
        let position = self.diatonic_position() + by.steps();
        Note {
            tpc,
            octave: (position - tpc.letter().index()).div_euclid(7),
            onset: self.onset,
            dur: self.dur,
        }
    }
}

/// A silence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rest {
    /// Whole notes from the start of the track.
    #[serde(with = "crate::duration::as_string")]
    pub onset: Duration,
    /// Length in whole notes.
    #[serde(with = "crate::duration::as_string")]
    pub dur: Duration,
}

impl Rest {
    /// Builds a rest.
    pub fn new(onset: Duration, dur: Duration) -> Rest {
        Rest { onset, dur }
    }

    /// Where the rest ends, in whole notes from the start.
    pub fn end(&self) -> Duration {
        self.onset + self.dur
    }
}

/// One entry on a track's timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Event {
    /// A sounding note.
    Note(Note),
    /// A silence.
    Rest(Rest),
}

impl Event {
    /// Whole notes from the start of the track.
    pub fn onset(&self) -> Duration {
        match self {
            Event::Note(note) => note.onset,
            Event::Rest(rest) => rest.onset,
        }
    }

    /// Length in whole notes.
    pub fn dur(&self) -> Duration {
        match self {
            Event::Note(note) => note.dur,
            Event::Rest(rest) => rest.dur,
        }
    }

    /// Where the event ends, in whole notes from the start.
    pub fn end(&self) -> Duration {
        self.onset() + self.dur()
    }

    /// The note, if this event sounds.
    pub fn as_note(&self) -> Option<&Note> {
        match self {
            Event::Note(note) => Some(note),
            Event::Rest(_) => None,
        }
    }

    /// Moves the event by an interval. Rests are unaffected — silence has no
    /// pitch to move.
    pub fn transpose(&self, by: Interval) -> Event {
        match self {
            Event::Note(note) => Event::Note(note.transpose(by)),
            Event::Rest(rest) => Event::Rest(*rest),
        }
    }
}

impl From<Note> for Event {
    fn from(note: Note) -> Event {
        Event::Note(note)
    }
}

impl From<Rest> for Event {
    fn from(rest: Rest) -> Event {
        Event::Rest(rest)
    }
}

/// A single monophonic voice.
///
/// One note sounds at a time. `tonic_tpc` is per-track on purpose: a B-flat
/// clarinet part and a capo-1 guitar part are the same mechanism.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// Display name, such as an instrument.
    pub name: String,
    /// The tonic this track's scale degrees are measured from.
    pub tonic_tpc: Tpc,
    /// The mode the tonic is read in.
    pub mode: Mode,
    /// The octave an unmarked scale degree sits in.
    ///
    /// Numbered notation writes a bare `5` with no octave mark, and something
    /// has to say which E-flat that is. This does: with `tonic_tpc` of A-flat
    /// and `register` 3, unmarked digits run A-flat 3 to G 4, so a bare `5` is
    /// E-flat 4 and `1^` is A-flat 4.
    ///
    /// [`Track::new`] picks it from the lowest note so that most music needs no
    /// marks below the line, and then it stays put. It is stored rather than
    /// recomputed on demand precisely so that adding one low note later does
    /// not silently flip every octave mark in the piece.
    pub register: i32,
    /// Events in timeline order, sorted by onset.
    pub events: Vec<Event>,
}

impl Track {
    /// Builds a track, sorting the events by onset.
    pub fn new(
        name: impl Into<String>,
        tonic_tpc: Tpc,
        mode: Mode,
        events: impl IntoIterator<Item = Event>,
    ) -> Track {
        let mut events: Vec<Event> = events.into_iter().collect();
        events.sort_by_key(|event| event.onset());
        let register = natural_register(tonic_tpc, &events);
        Track {
            name: name.into(),
            tonic_tpc,
            mode,
            register,
            events,
        }
    }

    /// Pins the octave that an unmarked scale degree sits in.
    ///
    /// Use this when the register is an authored decision rather than one
    /// inferred from the notes present.
    pub fn with_register(mut self, register: i32) -> Track {
        self.register = register;
        self
    }

    /// The tonic's position on the staff, which is where the register starts.
    pub fn register_position(&self) -> i32 {
        self.register * 7 + self.tonic_tpc.letter().index()
    }

    /// Splits the track into bar-sized pieces, filling holes with rests.
    ///
    /// Both renderings need exactly this: a note crossing a barline has to be
    /// drawn as two tied noteheads on a staff and as two `~`-joined symbols in
    /// numbered notation, and they must agree about where the split falls. So
    /// the split lives here, once, rather than in each renderer.
    ///
    /// A gap in the timeline is silence, and silence is a rest, so gaps come
    /// back as rests rather than vanishing. Returns nothing if `bar` is not a
    /// positive length.
    pub fn segments(&self, bar: Duration) -> Vec<Segment> {
        let mut segments = Vec::new();
        if bar <= duration::ZERO {
            return segments;
        }

        let mut cursor = duration::ZERO;
        for event in &self.events {
            if event.onset() > cursor {
                let hole = Rest::new(cursor, event.onset() - cursor);
                split_into_bars(&mut segments, Event::Rest(hole), bar);
            }
            split_into_bars(&mut segments, *event, bar);
            cursor = cursor.max(event.end());
        }
        segments
    }

    /// Where the last event ends, in whole notes. Zero for an empty track.
    pub fn length(&self) -> Duration {
        self.events
            .iter()
            .map(Event::end)
            .max()
            .unwrap_or(duration::ZERO)
    }

    /// Moves the whole track by an interval, tonic included.
    ///
    /// The tonic moves with the notes, so scale degrees come out unchanged —
    /// which is the point. Transposition is a relabelling, not a rewrite.
    pub fn transpose(&self, by: Interval) -> Track {
        let tonic_tpc = self.tonic_tpc.transpose(by);
        // The register moves with the music, exactly as a note does.
        let position = self.register_position() + by.steps();
        Track {
            name: self.name.clone(),
            tonic_tpc,
            mode: self.mode,
            register: (position - tonic_tpc.letter().index()).div_euclid(7),
            events: self
                .events
                .iter()
                .map(|event| event.transpose(by))
                .collect(),
        }
    }

    /// Moves the track into a new key by the shortest route.
    pub fn transposed_to(&self, tonic: Tpc) -> Track {
        self.transpose(Interval::between_tonics(self.tonic_tpc, tonic))
    }

    /// Reports every way this track's timeline is malformed.
    ///
    /// This is a query, not an assertion. A track under edit is legitimately
    /// incomplete between keystrokes; callers decide when a report matters —
    /// the editor surfaces it, export refuses on it.
    ///
    /// A note may cross a barline, since that is exactly what a tie is for.
    /// What may not happen is a gap, an overlap, or a final bar left part-full.
    pub fn validate(&self, time_sig: TimeSignature) -> Vec<TimelineIssue> {
        let mut issues = Vec::new();
        let mut cursor = duration::ZERO;

        for (index, event) in self.events.iter().enumerate() {
            if event.dur() <= duration::ZERO {
                issues.push(TimelineIssue::NonPositiveDuration {
                    index,
                    dur: event.dur(),
                });
            }
            if event.onset() > cursor {
                issues.push(TimelineIssue::Gap {
                    before: index,
                    from: cursor,
                    to: event.onset(),
                });
            } else if event.onset() < cursor {
                issues.push(TimelineIssue::Overlap {
                    index,
                    onset: event.onset(),
                    previous_end: cursor,
                });
            }
            cursor = cursor.max(event.end());
        }

        match time_sig.bar_length() {
            None => issues.push(TimelineIssue::InvalidTimeSignature { time_sig }),
            Some(bar) if !(cursor / bar).is_integer() => {
                issues.push(TimelineIssue::PartialFinalBar { total: cursor, bar });
            }
            Some(_) => {}
        }

        issues
    }
}

/// A score.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// Title, as displayed.
    pub title: String,
    /// Voices. The UI shows one for now; the model has never assumed that.
    pub tracks: Vec<Track>,
    /// Time signature, document-wide.
    pub time_sig: TimeSignature,
    /// Tempo in quarter notes per minute.
    pub bpm: u32,
}

impl Document {
    /// Builds a document.
    pub fn new(
        title: impl Into<String>,
        time_sig: TimeSignature,
        bpm: u32,
        tracks: impl IntoIterator<Item = Track>,
    ) -> Document {
        Document {
            title: title.into(),
            tracks: tracks.into_iter().collect(),
            time_sig,
            bpm,
        }
    }

    /// Moves every track by an interval.
    pub fn transpose(&self, by: Interval) -> Document {
        Document {
            title: self.title.clone(),
            tracks: self
                .tracks
                .iter()
                .map(|track| track.transpose(by))
                .collect(),
            time_sig: self.time_sig,
            bpm: self.bpm,
        }
    }

    /// Validates every track, pairing each issue with its track index.
    pub fn validate(&self) -> Vec<(usize, TimelineIssue)> {
        self.tracks
            .iter()
            .enumerate()
            .flat_map(|(index, track)| {
                track
                    .validate(self.time_sig)
                    .into_iter()
                    .map(move |issue| (index, issue))
            })
            .collect()
    }
}

/// One event, clipped to a single bar.
///
/// Produced by [`Track::segments`]. Every renderer consumes these rather than
/// raw events, so staff notation and numbered notation break notes across
/// barlines in the same places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Which bar this piece falls in, counting from zero.
    pub bar: i64,
    /// The piece itself, carrying its own onset and clipped length.
    pub event: Event,
    /// This piece continues a note that began in an earlier bar. On a staff it
    /// takes the far end of a tie; in numbered notation it takes a `~`.
    pub tied_to_previous: bool,
    /// This note continues past the barline into the next piece.
    pub tied_to_next: bool,
}

/// Cuts one event at every barline it crosses.
fn split_into_bars(out: &mut Vec<Segment>, event: Event, bar: Duration) {
    let end = event.end();
    let mut cursor = event.onset();
    let mut first = true;
    while cursor < end {
        let index = (cursor / bar).floor().to_integer();
        let bar_end = bar * (index + 1);
        let stop = if end < bar_end { end } else { bar_end };
        let piece = match event {
            Event::Note(note) => Event::Note(Note {
                onset: cursor,
                dur: stop - cursor,
                ..note
            }),
            Event::Rest(_) => Event::Rest(Rest::new(cursor, stop - cursor)),
        };
        out.push(Segment {
            bar: index,
            event: piece,
            tied_to_previous: !first,
            tied_to_next: stop < end,
        });
        cursor = stop;
        first = false;
    }
}

/// The register [`Track::new`] picks: low enough that no note needs a mark
/// below the line.
fn natural_register(tonic_tpc: Tpc, events: &[Event]) -> i32 {
    let lowest = events
        .iter()
        .filter_map(Event::as_note)
        .map(Note::diatonic_position)
        .min();
    match lowest {
        Some(position) => (position - tonic_tpc.letter().index()).div_euclid(7),
        None => 4,
    }
}

/// A way a track's timeline is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineIssue {
    /// Nothing fills the span before the event at `before`.
    Gap {
        /// Index of the event that starts after the hole.
        before: usize,
        /// Where the hole starts, in whole notes.
        from: Duration,
        /// Where the hole ends, in whole notes.
        to: Duration,
    },
    /// An event starts before the previous one has finished.
    Overlap {
        /// Index of the offending event.
        index: usize,
        /// Where it starts, in whole notes.
        onset: Duration,
        /// Where everything before it had already reached.
        previous_end: Duration,
    },
    /// An event has zero or negative length.
    NonPositiveDuration {
        /// Index of the offending event.
        index: usize,
        /// The length it claims.
        dur: Duration,
    },
    /// The music does not fill a whole number of bars.
    PartialFinalBar {
        /// Total length of the track, in whole notes.
        total: Duration,
        /// Length of one bar, in whole notes.
        bar: Duration,
    },
    /// The time signature cannot be interpreted.
    InvalidTimeSignature {
        /// The offending signature.
        time_sig: TimeSignature,
    },
}

impl fmt::Display for TimelineIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimelineIssue::Gap { before, from, to } => write!(
                f,
                "gap of {} whole notes at {} before event {before}",
                to - from,
                from
            ),
            TimelineIssue::Overlap {
                index,
                onset,
                previous_end,
            } => write!(
                f,
                "event {index} starts at {onset} but the previous event runs to {previous_end}"
            ),
            TimelineIssue::NonPositiveDuration { index, dur } => {
                write!(f, "event {index} has non-positive duration {dur}")
            }
            TimelineIssue::PartialFinalBar { total, bar } => write!(
                f,
                "track is {total} whole notes, not a whole number of {bar}-note bars"
            ),
            TimelineIssue::InvalidTimeSignature { time_sig } => {
                write!(f, "invalid time signature {time_sig}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::{EIGHTH, HALF, QUARTER, WHOLE};
    use crate::pitch::Letter;

    fn note_at(onset: Duration, dur: Duration) -> Event {
        Note::new(Tpc::natural(Letter::C), 4, onset, dur).into()
    }

    fn track_of(events: impl IntoIterator<Item = Event>) -> Track {
        Track::new("test", Tpc::C, Mode::Major, events)
    }

    #[test]
    fn middle_c_is_sixty() {
        let middle_c = Note::new(Tpc::C, 4, duration::ZERO, QUARTER);
        assert_eq!(middle_c.midi(), 60);
    }

    #[test]
    fn a_full_bar_validates_clean() {
        let track = track_of([note_at(duration::ZERO, HALF), note_at(HALF, HALF)]);
        assert_eq!(track.validate(TimeSignature::COMMON), []);
        assert_eq!(track.length(), WHOLE);
    }

    #[test]
    fn a_note_may_cross_a_barline_because_that_is_what_a_tie_is() {
        // A half note starting on the last quarter of bar 1: notated as two
        // tied quarters, stored as one note, and perfectly valid.
        let track = track_of([
            note_at(duration::ZERO, HALF + QUARTER),
            note_at(HALF + QUARTER, HALF),
            note_at(WHOLE + QUARTER, HALF + QUARTER),
        ]);
        assert_eq!(track.validate(TimeSignature::COMMON), []);
        assert_eq!(track.length(), WHOLE * 2);
    }

    #[test]
    fn gaps_overlaps_and_short_bars_are_reported() {
        let gapped = track_of([note_at(duration::ZERO, QUARTER), note_at(HALF, HALF)]);
        assert_eq!(
            gapped.validate(TimeSignature::COMMON),
            [TimelineIssue::Gap {
                before: 1,
                from: QUARTER,
                to: HALF,
            }]
        );

        let overlapped = track_of([
            note_at(duration::ZERO, HALF),
            note_at(QUARTER, HALF + EIGHTH),
        ]);
        assert_eq!(
            overlapped.validate(TimeSignature::COMMON),
            [
                TimelineIssue::Overlap {
                    index: 1,
                    onset: QUARTER,
                    previous_end: HALF,
                },
                TimelineIssue::PartialFinalBar {
                    total: HALF + QUARTER + EIGHTH,
                    bar: WHOLE,
                },
            ]
        );

        let short = track_of([note_at(duration::ZERO, HALF)]);
        assert_eq!(
            short.validate(TimeSignature::COMMON),
            [TimelineIssue::PartialFinalBar {
                total: HALF,
                bar: WHOLE,
            }]
        );
    }

    #[test]
    fn an_empty_track_is_valid() {
        assert_eq!(track_of([]).validate(TimeSignature::COMMON), []);
    }

    #[test]
    fn events_are_sorted_on_construction() {
        let track = track_of([note_at(HALF, HALF), note_at(duration::ZERO, HALF)]);
        assert_eq!(track.events[0].onset(), duration::ZERO);
        assert_eq!(track.validate(TimeSignature::COMMON), []);
    }
}
