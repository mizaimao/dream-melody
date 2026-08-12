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

//! Numbered notation (jianpu/简谱), as text.
//!
//! This is the format the model exists for. A degree is derived from the
//! distance between a note and its tonic along the line of fifths, which yields
//! the digit *and* its accidental with no heuristics and no table keyed on key
//! name. Transpose the music and the digits do not move — only the header does.
//!
//! # The text
//!
//! ```text
//! title: Fixture A
//! time: 4/4
//! tempo: 67
//!
//! track: Melody
//! 1 = Ab3
//! mode: Major
//! | 0_ 5_ 1^_ 5_ 1^_ 5_ 1^_ 2^_ | 4^_ 0_ 2^_ 5_ 4^ 2^_ 5_ |
//! ```
//!
//! `1 = Ab3` is how numbered notation states its key, and the octave says which
//! register an unmarked digit sits in.
//!
//! A symbol is `[accidental][digit][octave][duration]`:
//!
//! - **accidental** — `#` or `b`, repeated for doubles, before the digit.
//! - **digit** — `1` to `7`, or `0` for a rest.
//! - **octave** — `^` per octave up, `v` per octave down. Bare is the register
//!   the header names.
//! - **duration** — bare is one beat. `_` halves (so `_` is an eighth in 4/4,
//!   `__` a sixteenth). `.` dots. A following ` -` token adds a whole beat, so
//!   `1 - - -` fills a bar of 4/4. Anything these cannot express — a triplet,
//!   say — is written exactly as `[numerator/denominator]` in whole notes, so
//!   no duration is ever approximated.
//!
//! A `~` before a symbol ties it to the one before: same pitch, one sustained
//! note. That is the only thing `~` means, and it appears only where a note
//! crosses a barline, since the model stores such a note whole.

use core::fmt::Write as _;
use core::str::FromStr;

use num_rational::Ratio;

use crate::duration::Duration;
use crate::model::{Document, Event, Mode, Note, Rest, TimeSignature, Track};
use crate::pitch::Tpc;

/// A scale degree, as numbered notation writes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Degree {
    /// The digit, `1` through `7`.
    pub number: u32,
    /// Accidental applied to the degree: `-1` flat, `0` natural, `+1` sharp.
    pub alteration: i32,
    /// Octaves above the register the header names. Negative is below.
    pub octave: i32,
}

impl Degree {
    /// Derives the degree of a note in a key.
    ///
    /// `tonic_position` is the tonic's [`Note::diatonic_position`], which fixes
    /// the register an unmarked digit means.
    pub fn of(note: &Note, tonic_tpc: Tpc, tonic_position: i32) -> Degree {
        // The distance from the tonic along the line of fifths has exactly the
        // structure of a pitch: degree 1 is where C is, degree 4 where F is.
        // So the letter arithmetic already written does this job.
        let from_tonic = Tpc(note.tpc.0 - tonic_tpc.0);
        let index = from_tonic.letter().index();
        Degree {
            number: (index + 1) as u32,
            alteration: from_tonic.accidental(),
            octave: (note.diatonic_position() - tonic_position - index).div_euclid(7),
        }
    }

    /// The line-of-fifths distance from the tonic that this degree names.
    fn fifths_from_tonic(&self) -> i32 {
        let index = (self.number as i32 - 1).rem_euclid(7);
        Tpc::natural(crate::pitch::Letter::from_index(index)).0 + 7 * self.alteration
    }

    /// Rebuilds the note this degree names in a key.
    pub fn to_note(
        self,
        tonic_tpc: Tpc,
        tonic_position: i32,
        onset: Duration,
        dur: Duration,
    ) -> Note {
        let tpc = Tpc(tonic_tpc.0 + self.fifths_from_tonic());
        let index = (self.number as i32 - 1).rem_euclid(7);
        let position = tonic_position + index + 7 * self.octave;
        Note {
            tpc,
            octave: (position - tpc.letter().index()).div_euclid(7),
            onset,
            dur,
        }
    }
}

/// Why a document could not be written as, or read from, numbered notation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JianpuError {
    /// The time signature has no bar length, so bars cannot be drawn.
    InvalidTimeSignature(TimeSignature),
    /// The text does not say what it needs to.
    Syntax {
        /// Line number, counting from 1.
        line: usize,
        /// What was wrong.
        message: String,
    },
}

impl core::fmt::Display for JianpuError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            JianpuError::InvalidTimeSignature(time_sig) => {
                write!(f, "cannot draw bars in time signature {time_sig}")
            }
            JianpuError::Syntax { line, message } => write!(f, "line {line}: {message}"),
        }
    }
}

impl std::error::Error for JianpuError {}

fn syntax<T>(line: usize, message: impl Into<String>) -> Result<T, JianpuError> {
    Err(JianpuError::Syntax {
        line,
        message: message.into(),
    })
}

// --- durations -------------------------------------------------------------

/// The largest number of dots and beams worth spelling out. Beyond this the
/// exact form is both shorter and clearer.
const MAX_DOTS: u32 = 3;
const MAX_BEAMS: u32 = 6;

/// How numbered notation writes a length.
///
/// A digit sits on the beat. Beams under it halve that, dots after it lengthen
/// it by half, and dashes after it add whole beats. Anything those cannot say
/// exactly is stated as a fraction instead of being rounded to something close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurationMarks {
    /// Beams under the digit. One is half a beat, two is a quarter of one.
    pub beams: u32,
    /// Augmentation dots after the digit.
    pub dots: u32,
    /// Whole beats added after the digit, each drawn as a dash.
    pub dashes: u32,
    /// The exact length, when no combination of the above says it.
    pub exact: Option<Duration>,
}

impl DurationMarks {
    /// How numbered notation writes `dur`, given what one beat is worth.
    ///
    /// Both the text format and the renderer go through here, so a length is
    /// never written one way and drawn another.
    pub fn of(dur: Duration, beat: Duration) -> DurationMarks {
        let plain = DurationMarks {
            beams: 0,
            dots: 0,
            dashes: 0,
            exact: None,
        };
        let beats = dur / beat;

        if beats.is_integer() && *beats.numer() >= 1 {
            return DurationMarks {
                dashes: (beats.to_integer() - 1) as u32,
                ..plain
            };
        }

        for dots in 0..=MAX_DOTS {
            for beams in 0..=MAX_BEAMS {
                let numer = (1i64 << (dots + 1)) - 1;
                let denom = 1i64 << (dots + beams);
                if beats == Ratio::new(numer, denom) {
                    return DurationMarks {
                        beams,
                        dots,
                        ..plain
                    };
                }
            }
        }

        DurationMarks {
            exact: Some(dur),
            ..plain
        }
    }

    /// The marks as they attach to a digit in the text format.
    fn to_text(self) -> String {
        if let Some(exact) = self.exact {
            return format!("[{}/{}]", exact.numer(), exact.denom());
        }
        let mut marks = "_".repeat(self.beams as usize);
        marks.push_str(&".".repeat(self.dots as usize));
        marks
    }
}

/// Writes a duration as jianpu marks, given what one beat is worth.
///
/// Returns the marks that attach to the digit and how many ` -` tokens follow.
fn encode_duration(dur: Duration, beat: Duration) -> (String, u32) {
    let marks = DurationMarks::of(dur, beat);
    (marks.to_text(), marks.dashes)
}

/// Reads the duration marks attached to a digit.
fn decode_duration(marks: &str, beat: Duration, line: usize) -> Result<Duration, JianpuError> {
    if let Some(exact) = marks.strip_prefix('[') {
        let exact = match exact.strip_suffix(']') {
            Some(inner) => inner,
            None => return syntax(line, format!("unclosed exact duration \"[{exact}\"")),
        };
        return match Duration::from_str(exact) {
            Ok(value) if *value.numer() > 0 => Ok(value),
            _ => syntax(line, format!("\"{exact}\" is not a positive duration")),
        };
    }

    let halvings = marks.chars().take_while(|&mark| mark == '_').count();
    let dots = marks[halvings..]
        .chars()
        .take_while(|&mark| mark == '.')
        .count();
    if halvings + dots != marks.len() {
        return syntax(line, format!("\"{marks}\" is not a duration"));
    }
    if halvings as u32 > MAX_BEAMS || dots as u32 > MAX_DOTS {
        return syntax(line, format!("\"{marks}\" is longer than makes sense"));
    }

    let numer = (1i64 << (dots + 1)) - 1;
    let denom = 1i64 << (dots + halvings);
    Ok(beat * Ratio::new(numer, denom))
}

// --- writing ---------------------------------------------------------------

/// Writes a document as numbered notation.
///
/// Notes crossing a barline are split and re-joined with `~`, since the model
/// stores such a note whole and the notation cannot draw it that way.
pub fn to_string(document: &Document) -> Result<String, JianpuError> {
    let bar = document
        .time_sig
        .bar_length()
        .ok_or(JianpuError::InvalidTimeSignature(document.time_sig))?;
    let beat = bar / i64::from(document.time_sig.beats);

    let mut out = String::new();
    let _ = writeln!(out, "title: {}", document.title);
    let _ = writeln!(out, "time: {}", document.time_sig);
    let _ = writeln!(out, "tempo: {}", document.bpm);

    for track in &document.tracks {
        let _ = writeln!(out, "\ntrack: {}", track.name);
        let _ = writeln!(out, "1 = {}{}", track.tonic_tpc, track.register);
        let _ = writeln!(out, "mode: {}", track.mode);
        let _ = writeln!(out, "{}", write_body(track, bar, beat));
    }
    Ok(out)
}

fn write_body(track: &Track, bar: Duration, beat: Duration) -> String {
    let position = track.register_position();
    let segments = track.segments(bar);
    let bars = segments.last().map_or(0, |segment| segment.bar + 1);

    let mut out = String::from("|");
    for index in 0..bars {
        for segment in segments.iter().filter(|segment| segment.bar == index) {
            let (marks, dashes) = encode_duration(segment.event.dur(), beat);
            out.push(' ');
            match segment.event.as_note() {
                Some(note) => {
                    // Only a note can be tied; two rests in a row are just two
                    // rests, and `~0` would mean nothing.
                    if segment.tied_to_previous {
                        out.push('~');
                    }
                    let degree = Degree::of(note, track.tonic_tpc, position);
                    let accidental = if degree.alteration < 0 { "b" } else { "#" };
                    out.push_str(&accidental.repeat(degree.alteration.unsigned_abs() as usize));
                    let _ = write!(out, "{}", degree.number);
                    let mark = if degree.octave < 0 { "v" } else { "^" };
                    out.push_str(&mark.repeat(degree.octave.unsigned_abs() as usize));
                }
                None => out.push('0'),
            }
            out.push_str(&marks);
            for _ in 0..dashes {
                out.push_str(" -");
            }
        }
        out.push_str(" |");
    }
    out
}

// --- reading ---------------------------------------------------------------

/// Reads a document from numbered notation.
pub fn from_str(text: &str) -> Result<Document, JianpuError> {
    let mut title = String::new();
    let mut time_sig = TimeSignature::COMMON;
    let mut bpm = 0;
    let mut tracks: Vec<Track> = Vec::new();

    // Per-track state, flushed when the next `track:` or the end arrives.
    let mut name = String::new();
    let mut tonic = Tpc::C;
    let mut tonic_octave = 4;
    let mut mode = Mode::Major;
    let mut bodies: Vec<(usize, String)> = Vec::new();
    let mut started = false;

    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(value) = trimmed.strip_prefix("title:") {
            title = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("time:") {
            time_sig = parse_time_signature(value.trim(), line)?;
        } else if let Some(value) = trimmed.strip_prefix("tempo:") {
            bpm = match value.trim().parse() {
                Ok(parsed) => parsed,
                Err(_) => return syntax(line, format!("\"{}\" is not a tempo", value.trim())),
            };
        } else if let Some(value) = trimmed.strip_prefix("track:") {
            if started {
                tracks.push(build_track(
                    &name,
                    tonic,
                    tonic_octave,
                    mode,
                    &bodies,
                    time_sig,
                )?);
                bodies.clear();
            }
            started = true;
            name = value.trim().to_string();
            mode = Mode::Major;
        } else if let Some(value) = trimmed.strip_prefix("mode:") {
            mode = match Mode::from_name(value.trim()) {
                Some(parsed) => parsed,
                None => return syntax(line, format!("\"{}\" is not a mode", value.trim())),
            };
        } else if let Some(value) = trimmed.strip_prefix("1 =") {
            (tonic, tonic_octave) = parse_key(value.trim(), line)?;
        } else if trimmed.starts_with('|') {
            started = true;
            bodies.push((line, trimmed.to_string()));
        } else {
            return syntax(line, format!("cannot read \"{trimmed}\""));
        }
    }

    if started {
        tracks.push(build_track(
            &name,
            tonic,
            tonic_octave,
            mode,
            &bodies,
            time_sig,
        )?);
    }

    Ok(Document {
        title,
        tracks,
        time_sig,
        bpm,
    })
}

fn parse_time_signature(text: &str, line: usize) -> Result<TimeSignature, JianpuError> {
    match text.split_once('/') {
        Some((beats, beat_value)) => match (beats.trim().parse(), beat_value.trim().parse()) {
            (Ok(beats), Ok(beat_value)) => Ok(TimeSignature { beats, beat_value }),
            _ => syntax(line, format!("\"{text}\" is not a time signature")),
        },
        None => syntax(line, format!("\"{text}\" is not a time signature")),
    }
}

/// Reads `Ab3` into its spelling and its octave.
fn parse_key(text: &str, line: usize) -> Result<(Tpc, i32), JianpuError> {
    let split = text
        .find(|symbol: char| symbol.is_ascii_digit() || symbol == '-')
        .unwrap_or(text.len());
    let (spelling, octave) = text.split_at(split);
    match (Tpc::from_str(spelling), octave.parse()) {
        (Ok(tpc), Ok(octave)) => Ok((tpc, octave)),
        _ => syntax(line, format!("\"{text}\" is not a key such as \"Ab3\"")),
    }
}

fn build_track(
    name: &str,
    tonic: Tpc,
    tonic_octave: i32,
    mode: Mode,
    bodies: &[(usize, String)],
    time_sig: TimeSignature,
) -> Result<Track, JianpuError> {
    let bar = time_sig
        .bar_length()
        .ok_or(JianpuError::InvalidTimeSignature(time_sig))?;
    let beat = bar / i64::from(time_sig.beats);
    let position = tonic_octave * 7 + tonic.letter().index();

    let mut events: Vec<Event> = Vec::new();
    let mut cursor = Duration::from_integer(0);

    for (line, body) in bodies {
        for token in body.split_whitespace() {
            match token {
                "|" => continue,
                "-" => {
                    let last = match events.last_mut() {
                        Some(event) => event,
                        None => return syntax(*line, "a dash must follow a note or a rest"),
                    };
                    extend(last, beat);
                    cursor += beat;
                }
                _ => {
                    let (token, tied) = match token.strip_prefix('~') {
                        Some(rest) => (rest, true),
                        None => (token, false),
                    };
                    let (event, dur) = parse_symbol(token, tonic, position, cursor, beat, *line)?;
                    if tied {
                        tie_onto(&mut events, event, dur, *line)?;
                    } else {
                        events.push(event);
                    }
                    cursor += dur;
                }
            }
        }
    }

    Ok(Track {
        name: name.to_string(),
        tonic_tpc: tonic,
        mode,
        register: tonic_octave,
        events,
    })
}

/// Lengthens an event in place, which is what a dash and a tie both do.
fn extend(event: &mut Event, by: Duration) {
    match event {
        Event::Note(note) => note.dur += by,
        Event::Rest(rest) => rest.dur += by,
    }
}

/// Joins a tied symbol onto the note before it.
///
/// The pitches must match. Two noteheads of different pitch joined by a curve
/// are a slur, which means something else entirely and is not modelled.
fn tie_onto(
    events: &mut [Event],
    event: Event,
    dur: Duration,
    line: usize,
) -> Result<(), JianpuError> {
    let previous = match events.last_mut() {
        Some(previous) => previous,
        None => return syntax(line, "a tie must follow a note"),
    };
    match (previous.as_note().copied(), event.as_note()) {
        (Some(before), Some(after)) if before.tpc == after.tpc && before.octave == after.octave => {
            extend(previous, dur);
            Ok(())
        }
        (Some(before), Some(after)) => syntax(
            line,
            format!(
                "cannot tie {}{} to {}{}: a tie joins one pitch to itself, and two \
                 pitches joined by a curve is a slur, which this model does not keep",
                before.tpc, before.octave, after.tpc, after.octave
            ),
        ),
        _ => syntax(line, "a rest cannot be tied"),
    }
}

/// Reads one symbol: accidental, digit, octave marks, duration marks.
fn parse_symbol(
    token: &str,
    tonic: Tpc,
    position: i32,
    onset: Duration,
    beat: Duration,
    line: usize,
) -> Result<(Event, Duration), JianpuError> {
    let mut chars = token.char_indices().peekable();

    let mut alteration = 0;
    while let Some(&(_, symbol)) = chars.peek() {
        match symbol {
            'b' => alteration -= 1,
            '#' => alteration += 1,
            _ => break,
        }
        chars.next();
    }

    let number = match chars.next() {
        Some((_, digit)) if digit.is_ascii_digit() => digit as u32 - '0' as u32,
        _ => return syntax(line, format!("\"{token}\" does not name a degree")),
    };
    if number > 7 {
        return syntax(line, format!("\"{token}\" is not a degree; 0 to 7 only"));
    }

    let mut octave = 0;
    while let Some(&(_, symbol)) = chars.peek() {
        match symbol {
            '^' => octave += 1,
            'v' => octave -= 1,
            _ => break,
        }
        chars.next();
    }

    let marks = match chars.peek() {
        Some(&(at, _)) => &token[at..],
        None => "",
    };
    let dur = decode_duration(marks, beat, line)?;

    if number == 0 {
        if alteration != 0 || octave != 0 {
            return syntax(line, "a rest takes no accidental and no octave");
        }
        return Ok((Rest::new(onset, dur).into(), dur));
    }

    let degree = Degree {
        number,
        alteration,
        octave,
    };
    Ok((degree.to_note(tonic, position, onset, dur).into(), dur))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::{EIGHTH, QUARTER, WHOLE, ZERO};
    use crate::pitch::Letter;

    fn track_in(tonic: Tpc, events: impl IntoIterator<Item = Event>) -> Document {
        Document::new(
            "T",
            TimeSignature::COMMON,
            60,
            [Track::new("Melody", tonic, Mode::Major, events)],
        )
    }

    #[test]
    fn a_degree_is_the_distance_from_the_tonic_along_the_line_of_fifths() {
        // In A-flat major: Ab is 1, Bb is 2, Db is 4, Eb is 5.
        let tonic = Tpc::new(Letter::A, -1);
        let position = 3 * 7 + Letter::A.index();
        let degree_of = |tpc: Tpc, octave: i32| {
            Degree::of(&Note::new(tpc, octave, ZERO, QUARTER), tonic, position)
        };
        assert_eq!(degree_of(Tpc::new(Letter::A, -1), 3).number, 1);
        assert_eq!(degree_of(Tpc::new(Letter::B, -1), 3).number, 2);
        assert_eq!(degree_of(Tpc::new(Letter::D, -1), 4).number, 4);
        assert_eq!(degree_of(Tpc::new(Letter::E, -1), 4).number, 5);
        // A raised degree keeps its digit and gains an accidental.
        assert_eq!(degree_of(Tpc::new(Letter::D, 0), 4).number, 4);
        assert_eq!(degree_of(Tpc::new(Letter::D, 0), 4).alteration, 1);
        // Octave marks count from the register the tonic names.
        assert_eq!(degree_of(Tpc::new(Letter::A, -1), 4).octave, 1);
        assert_eq!(degree_of(Tpc::new(Letter::A, -1), 2).octave, -1);
    }

    #[test]
    fn durations_encode_the_way_the_notation_writes_them() {
        let beat = QUARTER;
        assert_eq!(encode_duration(QUARTER, beat), (String::new(), 0));
        assert_eq!(encode_duration(EIGHTH, beat), ("_".into(), 0));
        assert_eq!(encode_duration(EIGHTH / 2, beat), ("__".into(), 0));
        assert_eq!(encode_duration(WHOLE, beat), (String::new(), 3));
        assert_eq!(encode_duration(QUARTER * 3, beat), (String::new(), 2));
        assert_eq!(encode_duration(QUARTER + EIGHTH, beat), (".".into(), 0));
        assert_eq!(encode_duration(EIGHTH + EIGHTH / 2, beat), ("_.".into(), 0));
        // A triplet eighth cannot be spelled with dots and beams, so it is
        // written exactly rather than rounded.
        assert_eq!(
            encode_duration(Ratio::new(1, 12), beat),
            ("[1/12]".into(), 0)
        );
    }

    #[test]
    fn every_duration_survives_its_own_encoding() {
        let beat = QUARTER;
        let cases = [
            QUARTER,
            EIGHTH,
            EIGHTH / 2,
            QUARTER + EIGHTH,
            EIGHTH + EIGHTH / 2,
            Ratio::new(1, 12),
            Ratio::new(1, 20),
            Ratio::new(7, 32),
        ];
        for dur in cases {
            let (marks, dashes) = encode_duration(dur, beat);
            let decoded = decode_duration(&marks, beat, 1).unwrap() + beat * i64::from(dashes);
            assert_eq!(decoded, dur, "marks {marks:?}");
        }
    }

    #[test]
    fn a_tie_of_two_different_pitches_is_refused_as_a_slur() {
        let text = "\
title: T
time: 4/4
tempo: 60

track: Melody
1 = C4
mode: Major
| 1 ~2 - - |
";
        let error = from_str(text).unwrap_err();
        let JianpuError::Syntax { message, .. } = &error else {
            panic!("expected a syntax error, got {error:?}");
        };
        assert!(message.contains("slur"), "{message}");
    }

    #[test]
    fn a_rest_cannot_carry_a_pitch() {
        let document = track_in(Tpc::C, [Rest::new(ZERO, WHOLE).into()]);
        let text = to_string(&document).unwrap();
        assert!(text.contains("| 0 - - - |"), "{text}");
        assert_eq!(from_str(&text).unwrap(), document);
    }

    #[test]
    fn a_note_crossing_a_barline_is_split_and_tied() {
        // A half note starting on the last beat of bar 1.
        let document = track_in(
            Tpc::C,
            [
                Note::new(Tpc::C, 4, ZERO, QUARTER * 3).into(),
                Note::new(Tpc::C, 4, QUARTER * 3, QUARTER * 2).into(),
                Note::new(Tpc::C, 4, QUARTER * 5, QUARTER * 3).into(),
            ],
        );
        let text = to_string(&document).unwrap();
        // Bar 1 holds a dotted-half's worth plus the first beat of the half
        // note; the tie mark opens bar 2, where the note continues.
        assert!(text.contains("| 1 - - 1 | ~1 1 - - |"), "{text}");
        // And it comes back as one note, not two.
        let loaded = from_str(&text).unwrap();
        assert_eq!(loaded, document);
        assert_eq!(loaded.tracks[0].events.len(), 3);
    }
}
