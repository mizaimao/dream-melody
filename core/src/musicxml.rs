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

//! MusicXML, in both directions.
//!
//! This is how the project talks to the rest of the world. MuseScore, Sibelius,
//! Finale and Dorico all read it, and unlike MIDI it stores spelling — a
//! `<pitch>` is a step, an alter and an octave, which is exactly the
//! information a tonal pitch class holds. So A-flat crosses the boundary as
//! A-flat and comes back as A-flat.
//!
//! Only what this model holds is written: one voice per part, no dynamics, no
//! articulations. Anything else in an imported file is ignored rather than
//! guessed at.
//!
//! Ties are split on the way out and merged on the way back, since the model
//! stores a note whole and notation cannot draw one across a barline.

use core::fmt::Write as _;

use num_integer::lcm;
use num_rational::Ratio;

use crate::duration::{self, Duration};
use crate::model::{Document, Event, Mode, Note, Rest, TimeSignature, Track};
use crate::notevalue::NoteValue;
use crate::pitch::{Letter, Tpc};

/// Where the register is stashed, since MusicXML has no concept of one.
///
/// `<miscellaneous-field>` is the standard's own extension point: other
/// programs ignore what they do not recognise, so this costs nothing and buys
/// an exact round trip.
const REGISTER_FIELD: &str = "dream-melody-register";

/// Why a score could not be written as, or read from, MusicXML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusicXmlError {
    /// The time signature has no bar length.
    InvalidTimeSignature(TimeSignature),
    /// A duration needs finer division than MusicXML can carry sensibly.
    UndividableDuration(Duration),
    /// The XML is not well formed.
    Malformed(String),
    /// The XML is well formed but is not a score this can read.
    Unreadable(String),
}

impl core::fmt::Display for MusicXmlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MusicXmlError::InvalidTimeSignature(time_sig) => {
                write!(f, "cannot draw bars in time signature {time_sig}")
            }
            MusicXmlError::UndividableDuration(dur) => {
                write!(f, "{dur} whole notes needs more divisions than is sane")
            }
            MusicXmlError::Malformed(why) => write!(f, "malformed XML: {why}"),
            MusicXmlError::Unreadable(why) => write!(f, "not a score this can read: {why}"),
        }
    }
}

impl std::error::Error for MusicXmlError {}

// --- writing ---------------------------------------------------------------

/// Escapes the five characters XML reserves.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for symbol in text.chars() {
        match symbol {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(symbol),
        }
    }
    out
}

/// How many divisions per quarter note this document needs to stay exact.
///
/// MusicXML counts duration in integers, so the divisor has to clear every
/// denominator in the piece. Rounding here is what puts float drift into other
/// programs' exports; refusing is better.
fn divisions_for(document: &Document, bar: Duration) -> Result<i64, MusicXmlError> {
    let mut divisions: i64 = 1;
    for track in &document.tracks {
        for segment in track.segments(bar) {
            // A duration in quarter notes, whose denominator we must clear.
            let quarters = segment.event.dur() * 4;
            divisions = lcm(divisions, *quarters.denom());
            if divisions > 8064 {
                return Err(MusicXmlError::UndividableDuration(segment.event.dur()));
            }
        }
    }
    Ok(divisions)
}

/// Writes a document as MusicXML 4.0, partwise.
pub fn to_string(document: &Document) -> Result<String, MusicXmlError> {
    let bar = document
        .time_sig
        .bar_length()
        .ok_or(MusicXmlError::InvalidTimeSignature(document.time_sig))?;
    let divisions = divisions_for(document, bar)?;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \
         \"http://www.musicxml.org/dtds/partwise.dtd\">\n",
    );
    out.push_str("<score-partwise version=\"4.0\">\n");
    let _ = writeln!(
        out,
        "  <work><work-title>{}</work-title></work>",
        escape(&document.title)
    );

    out.push_str("  <identification>\n    <miscellaneous>\n");
    for (index, track) in document.tracks.iter().enumerate() {
        let _ = writeln!(
            out,
            "      <miscellaneous-field name=\"{}-{}\">{}</miscellaneous-field>",
            REGISTER_FIELD,
            index + 1,
            track.register
        );
    }
    out.push_str("    </miscellaneous>\n  </identification>\n");

    out.push_str("  <part-list>\n");
    for (index, track) in document.tracks.iter().enumerate() {
        let _ = writeln!(
            out,
            "    <score-part id=\"P{}\"><part-name>{}</part-name></score-part>",
            index + 1,
            escape(&track.name)
        );
    }
    out.push_str("  </part-list>\n");

    for (index, track) in document.tracks.iter().enumerate() {
        let _ = writeln!(out, "  <part id=\"P{}\">", index + 1);
        write_part(&mut out, document, track, bar, divisions);
        out.push_str("  </part>\n");
    }

    out.push_str("</score-partwise>\n");
    Ok(out)
}

fn write_part(out: &mut String, document: &Document, track: &Track, bar: Duration, divisions: i64) {
    let segments = track.segments(bar);
    let bars = segments.last().map_or(1, |segment| segment.bar + 1);

    for index in 0..bars {
        let _ = writeln!(out, "    <measure number=\"{}\">", index + 1);
        if index == 0 {
            let _ = writeln!(
                out,
                "      <attributes>\n        <divisions>{divisions}</divisions>\n        \
                 <key><fifths>{}</fifths><mode>{}</mode></key>\n        \
                 <time><beats>{}</beats><beat-type>{}</beat-type></time>\n        \
                 <clef><sign>G</sign><line>2</line></clef>\n      </attributes>",
                track.key_fifths(),
                track.mode.name().to_lowercase(),
                document.time_sig.beats,
                document.time_sig.beat_value,
            );
            let _ = writeln!(
                out,
                "      <direction placement=\"above\">\n        <direction-type>\n          \
                 <metronome><beat-unit>quarter</beat-unit>\
                 <per-minute>{bpm}</per-minute></metronome>\n        </direction-type>\n        \
                 <sound tempo=\"{bpm}\"/>\n      </direction>",
                bpm = document.bpm,
            );
        }

        for segment in segments.iter().filter(|segment| segment.bar == index) {
            write_note(out, segment, divisions, track.key_fifths());
        }
        out.push_str("    </measure>\n");
    }
}

fn write_note(out: &mut String, segment: &crate::model::Segment, divisions: i64, key_fifths: i32) {
    let dur = segment.event.dur();
    let ticks = (dur * 4 * divisions).to_integer();

    out.push_str("      <note>\n");
    match segment.event.as_note() {
        None => out.push_str("        <rest/>\n"),
        Some(note) => {
            let _ = writeln!(
                out,
                "        <pitch><step>{}</step><alter>{}</alter>\
                 <octave>{}</octave></pitch>",
                note.tpc.letter(),
                note.tpc.accidental(),
                note.octave
            );
        }
    }
    let _ = writeln!(out, "        <duration>{ticks}</duration>");

    if segment.tied_to_previous {
        out.push_str("        <tie type=\"stop\"/>\n");
    }
    if segment.tied_to_next {
        out.push_str("        <tie type=\"start\"/>\n");
    }

    if let Some(value) = NoteValue::of(dur) {
        let _ = writeln!(out, "        <type>{}</type>", value.head);
        for _ in 0..value.dots {
            out.push_str("        <dot/>\n");
        }
        if value.is_tuplet() {
            let _ = writeln!(
                out,
                "        <time-modification><actual-notes>{}</actual-notes>\
                 <normal-notes>{}</normal-notes></time-modification>",
                value.actual, value.normal
            );
        }
    }

    // An accidental is drawn only where it differs from the key signature.
    if let Some(note) = segment.event.as_note()
        && !segment.tied_to_previous
        && Tpc::in_key(note.tpc.letter(), key_fifths) != note.tpc
        && let Some(name) = accidental_name(note.tpc.accidental())
    {
        let _ = writeln!(out, "        <accidental>{name}</accidental>");
    }

    if segment.tied_to_previous || segment.tied_to_next {
        out.push_str("        <notations>\n");
        if segment.tied_to_previous {
            out.push_str("          <tied type=\"stop\"/>\n");
        }
        if segment.tied_to_next {
            out.push_str("          <tied type=\"start\"/>\n");
        }
        out.push_str("        </notations>\n");
    }
    out.push_str("      </note>\n");
}

fn accidental_name(accidental: i32) -> Option<&'static str> {
    Some(match accidental {
        -2 => "flat-flat",
        -1 => "flat",
        0 => "natural",
        1 => "sharp",
        2 => "double-sharp",
        _ => return None,
    })
}

// --- reading ---------------------------------------------------------------

/// Reads a document from MusicXML.
///
/// Understands score-partwise, which is what every program writes. Anything
/// the model does not hold — dynamics, lyrics, several voices in one part — is
/// ignored rather than approximated.
pub fn from_str(xml: &str) -> Result<Document, MusicXmlError> {
    // Every MusicXML file in the wild carries a DOCTYPE pointing at the
    // Recordare DTD, so it has to be allowed. Nothing is fetched: roxmltree
    // does not resolve external entities.
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let parsed = roxmltree::Document::parse_with_options(xml, options)
        .map_err(|error| MusicXmlError::Malformed(error.to_string()))?;
    let root = parsed.root_element();
    if !root.has_tag_name("score-partwise") {
        return Err(MusicXmlError::Unreadable(format!(
            "expected a score-partwise root, found <{}>",
            root.tag_name().name()
        )));
    }

    let title = find_text(root, &["work", "work-title"])
        .or_else(|| find_text(root, &["movement-title"]))
        .unwrap_or_default();

    let names = part_names(root);
    let registers = registers(root);

    let mut time_sig = TimeSignature::COMMON;
    let mut bpm = 0;
    let mut tracks = Vec::new();

    for (index, part) in root
        .children()
        .filter(|node| node.has_tag_name("part"))
        .enumerate()
    {
        let (track, part_time_sig, part_bpm) = read_part(part, index, &names, &registers)?;
        if let Some(found) = part_time_sig {
            time_sig = found;
        }
        if let Some(found) = part_bpm {
            bpm = found;
        }
        tracks.push(track);
    }

    if tracks.is_empty() {
        return Err(MusicXmlError::Unreadable("the score has no parts".into()));
    }

    Ok(Document {
        title,
        tracks,
        time_sig,
        bpm,
    })
}

fn find_text<'a>(node: roxmltree::Node<'a, 'a>, path: &[&str]) -> Option<String> {
    let mut current = node;
    for step in path {
        current = current.children().find(|child| child.has_tag_name(*step))?;
    }
    current.text().map(|text| text.trim().to_string())
}

fn find_number<T: core::str::FromStr>(node: roxmltree::Node<'_, '_>, tag: &str) -> Option<T> {
    find_text(node, &[tag])?.parse().ok()
}

fn part_names(root: roxmltree::Node<'_, '_>) -> Vec<String> {
    root.children()
        .find(|node| node.has_tag_name("part-list"))
        .into_iter()
        .flat_map(|list| {
            list.children()
                .filter(|node| node.has_tag_name("score-part"))
        })
        .map(|part| find_text(part, &["part-name"]).unwrap_or_default())
        .collect()
}

/// Recovers the registers this program stashed, if this file came from here.
fn registers(root: roxmltree::Node<'_, '_>) -> Vec<Option<i32>> {
    let mut found = Vec::new();
    let fields = root
        .descendants()
        .filter(|node| node.has_tag_name("miscellaneous-field"));
    for field in fields {
        let Some(name) = field.attribute("name") else {
            continue;
        };
        let Some(index) = name
            .strip_prefix(REGISTER_FIELD)
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|number| number.parse::<usize>().ok())
        else {
            continue;
        };
        let value = field.text().and_then(|text| text.trim().parse().ok());
        if found.len() < index {
            found.resize(index, None);
        }
        found[index - 1] = value;
    }
    found
}

type PartRead = (Track, Option<TimeSignature>, Option<u32>);

fn read_part(
    part: roxmltree::Node<'_, '_>,
    index: usize,
    names: &[String],
    registers: &[Option<i32>],
) -> Result<PartRead, MusicXmlError> {
    let mut divisions: i64 = 1;
    let mut key_fifths = 0;
    let mut mode = Mode::Major;
    let mut time_sig = None;
    let mut bpm = None;

    let mut events: Vec<Event> = Vec::new();
    let mut cursor = duration::ZERO;

    for measure in part.children().filter(|node| node.has_tag_name("measure")) {
        for node in measure.children() {
            if node.has_tag_name("attributes") {
                if let Some(found) = find_number::<i64>(node, "divisions")
                    && found > 0
                {
                    divisions = found;
                }
                if let Some(key) = node.children().find(|child| child.has_tag_name("key")) {
                    if let Some(found) = find_number::<i32>(key, "fifths") {
                        key_fifths = found;
                    }
                    if let Some(name) = find_text(key, &["mode"])
                        && let Some(found) = read_mode(&name)
                    {
                        mode = found;
                    }
                }
                if let Some(time) = node.children().find(|child| child.has_tag_name("time"))
                    && let (Some(beats), Some(beat_value)) = (
                        find_number::<u32>(time, "beats"),
                        find_number::<u32>(time, "beat-type"),
                    )
                {
                    time_sig = Some(TimeSignature { beats, beat_value });
                }
            } else if node.has_tag_name("direction") {
                if let Some(sound) = node.descendants().find(|child| child.has_tag_name("sound"))
                    && let Some(tempo) = sound.attribute("tempo")
                    && let Ok(parsed) = tempo.parse::<f64>()
                {
                    bpm = Some(parsed.round() as u32);
                } else if let Some(found) = node
                    .descendants()
                    .find(|child| child.has_tag_name("per-minute"))
                    .and_then(|per| per.text())
                    .and_then(|text| text.trim().parse::<f64>().ok())
                {
                    bpm = Some(found.round() as u32);
                }
            } else if node.has_tag_name("note") {
                read_note(node, divisions, key_fifths, &mut events, &mut cursor)?;
            } else if node.has_tag_name("backup") || node.has_tag_name("forward") {
                return Err(MusicXmlError::Unreadable(
                    "the part has more than one voice, which this model does not hold".into(),
                ));
            }
        }
    }

    let name = names.get(index).cloned().unwrap_or_default();
    let tonic_tpc = Tpc(key_fifths + mode.fifths_offset());
    let track = Track::new(name, tonic_tpc, mode, events);
    Ok(match registers.get(index).copied().flatten() {
        Some(register) => (track.with_register(register), time_sig, bpm),
        None => (track, time_sig, bpm),
    })
}

fn read_mode(name: &str) -> Option<Mode> {
    match name.trim().to_ascii_lowercase().as_str() {
        "major" | "ionian" => Some(Mode::Major),
        "minor" | "aeolian" => Some(Mode::Minor),
        other => Mode::from_name(other),
    }
}

fn read_note(
    node: roxmltree::Node<'_, '_>,
    divisions: i64,
    key_fifths: i32,
    events: &mut Vec<Event>,
    cursor: &mut Duration,
) -> Result<(), MusicXmlError> {
    // A chord note sounds with the one before rather than after it. This model
    // holds one note at a time, so the extra voice is dropped.
    if node.children().any(|child| child.has_tag_name("chord")) {
        return Ok(());
    }

    let ticks: i64 = find_number(node, "duration").unwrap_or(0);
    let dur = Ratio::new(ticks, divisions * 4);
    if dur <= duration::ZERO {
        return Ok(());
    }

    let is_rest = node.children().any(|child| child.has_tag_name("rest"));
    if is_rest {
        events.push(Rest::new(*cursor, dur).into());
        *cursor += dur;
        return Ok(());
    }

    let pitch = node
        .children()
        .find(|child| child.has_tag_name("pitch"))
        .ok_or_else(|| MusicXmlError::Unreadable("a note with neither pitch nor rest".into()))?;

    let step = find_text(pitch, &["step"])
        .ok_or_else(|| MusicXmlError::Unreadable("a pitch with no step".into()))?;
    let letter = read_step(&step)
        .ok_or_else(|| MusicXmlError::Unreadable(format!("\"{step}\" is not a step")))?;
    let octave: i32 = find_number(pitch, "octave")
        .ok_or_else(|| MusicXmlError::Unreadable("a pitch with no octave".into()))?;
    // `alter` is optional and, in some exports, fractional for microtones. Only
    // whole semitones have a spelling, so anything else falls back on the key.
    let alter = find_text(pitch, &["alter"])
        .and_then(|text| text.parse::<f64>().ok())
        .map(|value| value.round() as i32)
        .unwrap_or_else(|| Tpc::in_key(letter, key_fifths).accidental());

    let tied_stop = node
        .children()
        .filter(|child| child.has_tag_name("tie"))
        .any(|tie| tie.attribute("type") == Some("stop"));

    let note = Note::new(Tpc::new(letter, alter), octave, *cursor, dur);
    // A tie joins one pitch to itself, so the two noteheads become one note.
    // Anything else with a curve on it was a slur and is not kept.
    if tied_stop
        && let Some(Event::Note(previous)) = events.last_mut()
        && previous.tpc == note.tpc
        && previous.octave == note.octave
    {
        previous.dur += dur;
    } else {
        events.push(note.into());
    }
    *cursor += dur;
    Ok(())
}

fn read_step(step: &str) -> Option<Letter> {
    Some(match step.trim().to_ascii_uppercase().as_str() {
        "C" => Letter::C,
        "D" => Letter::D,
        "E" => Letter::E,
        "F" => Letter::F,
        "G" => Letter::G,
        "A" => Letter::A,
        "B" => Letter::B,
        _ => return None,
    })
}
