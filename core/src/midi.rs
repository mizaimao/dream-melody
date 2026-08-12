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

//! Standard MIDI files, in both directions.
//!
//! **MIDI cannot store spelling.** A-flat and G-sharp are the same byte, so
//! the distinction this whole model exists to keep is the one thing MIDI drops.
//! Reading a file back therefore spells each note to suit the key signature —
//! which recovers the original exactly for music that stays in its key, and
//! guesses for music that does not. Use MusicXML when spelling must survive.
//!
//! What MIDI *does* keep exactly is time, and that is Invariant 4. Ticks per
//! quarter note are chosen so every duration in the document lands on a whole
//! tick. Nothing is rounded; a document too finely divided is refused instead.

use num_integer::lcm;
use num_rational::Ratio;

use crate::duration::{self, Duration};
use crate::model::{Document, Event, Mode, Note, Rest, TimeSignature, Track};
use crate::pitch::Tpc;

/// The velocity every note is written with. There are no dynamics to express.
const VELOCITY: u8 = 80;
/// The largest ticks-per-quarter a standard MIDI file can hold.
const MAX_TICKS_PER_QUARTER: i64 = 32767;

/// Why a document could not be written as, or read from, MIDI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MidiError {
    /// A duration needs finer division than a MIDI file can carry.
    UndividableDuration(Duration),
    /// The bytes are not a standard MIDI file.
    Malformed(String),
    /// The file uses a feature this cannot read, such as SMPTE timing.
    Unsupported(String),
}

impl core::fmt::Display for MidiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MidiError::UndividableDuration(dur) => write!(
                f,
                "{dur} whole notes cannot be placed on a whole tick without rounding"
            ),
            MidiError::Malformed(why) => write!(f, "not a MIDI file: {why}"),
            MidiError::Unsupported(why) => write!(f, "unsupported MIDI file: {why}"),
        }
    }
}

impl std::error::Error for MidiError {}

// --- writing ---------------------------------------------------------------

/// Writes a value as MIDI's variable-length quantity.
fn write_varlen(out: &mut Vec<u8>, mut value: u32) {
    let mut buffer = [0u8; 4];
    let mut length = 0;
    loop {
        buffer[length] = (value & 0x7f) as u8;
        length += 1;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    for index in (0..length).rev() {
        let continues = if index > 0 { 0x80 } else { 0x00 };
        out.push(buffer[index] | continues);
    }
}

fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(tag);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
}

/// Ticks per quarter note fine enough to place every event exactly.
fn ticks_per_quarter(document: &Document) -> Result<i64, MidiError> {
    let mut ticks: i64 = 1;
    for track in &document.tracks {
        for event in &track.events {
            for value in [event.onset(), event.dur()] {
                // In quarter notes, so the denominator is what must be cleared.
                let quarters = value * 4;
                ticks = lcm(ticks, *quarters.denom());
                if ticks > MAX_TICKS_PER_QUARTER {
                    return Err(MidiError::UndividableDuration(event.dur()));
                }
            }
        }
    }
    // Scale up to something conventional so other programs are not surprised
    // by a resolution of 2. Doubling keeps every event exact.
    while ticks < 96 && ticks * 2 <= MAX_TICKS_PER_QUARTER {
        ticks *= 2;
    }
    Ok(ticks)
}

/// Writes a document as a format 1 standard MIDI file.
pub fn to_bytes(document: &Document) -> Result<Vec<u8>, MidiError> {
    let ticks = ticks_per_quarter(document)?;
    let mut out = Vec::new();

    let mut header = Vec::new();
    header.extend_from_slice(&1u16.to_be_bytes());
    header.extend_from_slice(&((document.tracks.len() + 1) as u16).to_be_bytes());
    header.extend_from_slice(&(ticks as u16).to_be_bytes());
    write_chunk(&mut out, b"MThd", &header);

    write_chunk(&mut out, b"MTrk", &conductor_track(document));
    for track in &document.tracks {
        write_chunk(&mut out, b"MTrk", &music_track(track, ticks));
    }
    Ok(out)
}

/// Track zero: the tempo, the time signature, and the key signature.
fn conductor_track(document: &Document) -> Vec<u8> {
    let mut body = Vec::new();

    write_varlen(&mut body, 0);
    body.extend_from_slice(&[0xff, 0x03]);
    let title = document.title.as_bytes();
    write_varlen(&mut body, title.len() as u32);
    body.extend_from_slice(title);

    // Microseconds per quarter note. A tempo of 67 does not divide evenly, so
    // this is the one place a value is rounded — and reading rounds back.
    let micros = (60_000_000f64 / f64::from(document.bpm.max(1))).round() as u32;
    write_varlen(&mut body, 0);
    body.extend_from_slice(&[0xff, 0x51, 0x03]);
    body.extend_from_slice(&micros.to_be_bytes()[1..]);

    write_varlen(&mut body, 0);
    let denominator_power = document.time_sig.beat_value.max(1).ilog2() as u8;
    body.extend_from_slice(&[
        0xff,
        0x58,
        0x04,
        document.time_sig.beats as u8,
        denominator_power,
        24,
        8,
    ]);

    // A MIDI key signature runs from seven flats to seven sharps. F-flat major
    // is eight flats and simply cannot be said here, so it is left out rather
    // than written wrong.
    if let Some(track) = document.tracks.first() {
        let fifths = track.key_fifths();
        if (-7..=7).contains(&fifths) {
            let minor = u8::from(matches!(track.mode, Mode::Minor));
            write_varlen(&mut body, 0);
            body.extend_from_slice(&[0xff, 0x59, 0x02, fifths as i8 as u8, minor]);
        }
    }

    write_varlen(&mut body, 0);
    body.extend_from_slice(&[0xff, 0x2f, 0x00]);
    body
}

fn music_track(track: &Track, ticks: i64) -> Vec<u8> {
    let mut body = Vec::new();

    write_varlen(&mut body, 0);
    body.extend_from_slice(&[0xff, 0x03]);
    let name = track.name.as_bytes();
    write_varlen(&mut body, name.len() as u32);
    body.extend_from_slice(name);

    // (tick, is_note_off, key). Note-offs sort before note-ons at the same
    // tick, so a repeated pitch is released before it is struck again.
    let mut moments: Vec<(i64, bool, u8)> = Vec::new();
    for note in track.events.iter().filter_map(Event::as_note) {
        let start = (note.onset * 4 * ticks).to_integer();
        let stop = (note.end() * 4 * ticks).to_integer();
        let key = note.midi().clamp(0, 127) as u8;
        moments.push((start, false, key));
        moments.push((stop, true, key));
    }
    moments.sort_by_key(|&(tick, is_off, key)| (tick, !is_off, key));

    let mut previous = 0;
    for (tick, is_off, key) in moments {
        write_varlen(&mut body, (tick - previous) as u32);
        body.push(if is_off { 0x80 } else { 0x90 });
        body.push(key);
        body.push(if is_off { 0 } else { VELOCITY });
        previous = tick;
    }

    write_varlen(&mut body, 0);
    body.extend_from_slice(&[0xff, 0x2f, 0x00]);
    body
}

// --- reading ---------------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0 }
    }

    fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }

    fn byte(&mut self) -> Result<u8, MidiError> {
        let found = self
            .bytes
            .get(self.at)
            .copied()
            .ok_or_else(|| MidiError::Malformed("the file ends mid-event".into()))?;
        self.at += 1;
        Ok(found)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], MidiError> {
        let end = self
            .at
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| MidiError::Malformed("a chunk runs past the end".into()))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn be_u16(&mut self) -> Result<u16, MidiError> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn be_u32(&mut self) -> Result<u32, MidiError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn varlen(&mut self) -> Result<u32, MidiError> {
        let mut value: u32 = 0;
        for _ in 0..4 {
            let byte = self.byte()?;
            value = (value << 7) | u32::from(byte & 0x7f);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(MidiError::Malformed("a length runs past four bytes".into()))
    }
}

/// A note read back from a file: when it starts, how long it lasts, and which
/// key was struck. The spelling is not here because MIDI does not have it.
type TimedNote = (Duration, Duration, i32);

/// A track's name and the notes read from it.
type TrackRead = (String, Vec<TimedNote>);

/// What the conductor track said.
struct Conductor {
    title: String,
    bpm: u32,
    time_sig: TimeSignature,
    key_fifths: i32,
    mode: Mode,
}

impl Default for Conductor {
    fn default() -> Conductor {
        Conductor {
            title: String::new(),
            bpm: 120,
            time_sig: TimeSignature::COMMON,
            key_fifths: 0,
            mode: Mode::Major,
        }
    }
}

/// Reads a standard MIDI file.
///
/// Spelling is not in the file, so notes are spelled to suit the key signature
/// the file declares. Music that stays in its key comes back exactly as it went
/// out; music that does not is spelled plausibly rather than correctly, because
/// the information is simply gone.
pub fn from_bytes(bytes: &[u8]) -> Result<Document, MidiError> {
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != b"MThd" {
        return Err(MidiError::Malformed("no MThd header".into()));
    }
    let header_length = reader.be_u32()?;
    let _format = reader.be_u16()?;
    let _track_count = reader.be_u16()?;
    let division = reader.be_u16()?;
    if header_length > 6 {
        reader.take(header_length as usize - 6)?;
    }
    if division & 0x8000 != 0 {
        return Err(MidiError::Unsupported(
            "SMPTE timing, which has no musical tick".into(),
        ));
    }
    let ticks = i64::from(division);
    if ticks == 0 {
        return Err(MidiError::Malformed("zero ticks per quarter note".into()));
    }

    let mut conductor = Conductor::default();
    let mut parts: Vec<TrackRead> = Vec::new();

    while !reader.done() {
        let tag = reader.take(4)?;
        let length = reader.be_u32()? as usize;
        let body = reader.take(length)?;
        if tag != b"MTrk" {
            continue;
        }
        let (name, notes) = read_track(body, ticks, &mut conductor)?;
        if !notes.is_empty() {
            parts.push((name, notes));
        }
    }

    let tonic_tpc = Tpc(conductor.key_fifths + conductor.mode.fifths_offset());
    let tracks: Vec<Track> = parts
        .into_iter()
        .map(|(name, notes)| {
            let mut events: Vec<Event> = Vec::new();
            let mut cursor = duration::ZERO;
            for (onset, dur, key) in notes {
                if onset > cursor {
                    // Silence between notes is a rest, so the timeline tiles.
                    events.push(Rest::new(cursor, onset - cursor).into());
                }
                let tpc = Tpc::nearest_in_key(key.rem_euclid(12), conductor.key_fifths);
                let octave = (key - tpc.letter().semitone() - tpc.accidental()).div_euclid(12) - 1;
                events.push(Note::new(tpc, octave, onset, dur).into());
                cursor = onset + dur;
            }
            Track::new(name, tonic_tpc, conductor.mode, events)
        })
        .collect();

    Ok(Document {
        title: conductor.title,
        tracks,
        time_sig: conductor.time_sig,
        bpm: conductor.bpm,
    })
}

fn read_track(body: &[u8], ticks: i64, conductor: &mut Conductor) -> Result<TrackRead, MidiError> {
    let mut reader = Reader::new(body);
    let mut tick: i64 = 0;
    let mut status: u8 = 0;
    let mut name = String::new();
    let mut sounding: Vec<(i64, i32)> = Vec::new();
    let mut notes: Vec<(i64, i64, i32)> = Vec::new();

    while !reader.done() {
        tick += i64::from(reader.varlen()?);
        let mut byte = reader.byte()?;
        if byte < 0x80 {
            // Running status: the previous command repeats.
            reader.at -= 1;
            byte = status;
        } else if byte < 0xf0 {
            status = byte;
        }

        match byte & 0xf0 {
            0x80 | 0x90 => {
                let key = i32::from(reader.byte()?);
                let velocity = reader.byte()?;
                let is_on = byte & 0xf0 == 0x90 && velocity > 0;
                if is_on {
                    sounding.push((tick, key));
                } else if let Some(index) =
                    sounding.iter().rposition(|&(_, sounded)| sounded == key)
                {
                    let (start, _) = sounding.remove(index);
                    notes.push((start, tick - start, key));
                }
            }
            0xa0 | 0xb0 | 0xe0 => {
                reader.take(2)?;
            }
            0xc0 | 0xd0 => {
                reader.take(1)?;
            }
            _ => match byte {
                0xff => {
                    let kind = reader.byte()?;
                    let length = reader.varlen()? as usize;
                    let data = reader.take(length)?;
                    read_meta(kind, data, conductor, &mut name);
                }
                0xf0 | 0xf7 => {
                    let length = reader.varlen()? as usize;
                    reader.take(length)?;
                }
                _ => return Err(MidiError::Malformed(format!("unknown status {byte:#04x}"))),
            },
        }
    }

    notes.sort_by_key(|&(start, _, key)| (start, key));
    let scale = Ratio::new(1, ticks * 4);
    let timed = notes
        .into_iter()
        .map(|(start, length, key)| (scale * start, scale * length, key))
        .collect();
    Ok((name, timed))
}

fn read_meta(kind: u8, data: &[u8], conductor: &mut Conductor, name: &mut String) {
    match kind {
        0x03 => {
            let text = String::from_utf8_lossy(data).into_owned();
            if conductor.title.is_empty() && name.is_empty() {
                // The first track name met is the conductor track's, which
                // carries the title.
                conductor.title = text;
            } else {
                *name = text;
            }
        }
        0x51 if data.len() == 3 => {
            let micros = u32::from(data[0]) << 16 | u32::from(data[1]) << 8 | u32::from(data[2]);
            if micros > 0 {
                conductor.bpm = (60_000_000f64 / f64::from(micros)).round() as u32;
            }
        }
        0x58 if data.len() >= 2 => {
            conductor.time_sig = TimeSignature {
                beats: u32::from(data[0]),
                beat_value: 1u32 << data[1].min(7),
            };
        }
        0x59 if data.len() == 2 => {
            conductor.key_fifths = i32::from(data[0] as i8);
            conductor.mode = if data[1] == 1 {
                Mode::Minor
            } else {
                Mode::Major
            };
        }
        _ => {}
    }
}
