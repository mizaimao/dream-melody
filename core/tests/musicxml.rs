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

//! MusicXML in both directions — the interchange other programs read.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::duration::{QUARTER, ZERO};
use dream_melody_core::interval::Interval;
use dream_melody_core::model::{Document, Mode, Note, TimeSignature, Track};
use dream_melody_core::musicxml;
use dream_melody_core::pitch::{Letter, Tpc};

use support::fixture_a;

#[test]
fn fixture_a_round_trips() {
    let document = fixture_a();
    let xml = musicxml::to_string(&document).unwrap();
    assert_eq!(musicxml::from_str(&xml).unwrap(), document);
}

/// The reason MusicXML is worth having: it stores spelling, so a flat key stays
/// a flat key instead of collapsing to sharps the way MIDI must.
#[test]
fn spelling_survives_in_every_key() {
    let source = fixture_a();
    for fifths in -12..=12 {
        let document = source.transpose(Interval::simplest(fifths));
        let xml = musicxml::to_string(&document).unwrap();
        let returned = musicxml::from_str(&xml).unwrap();
        assert_eq!(returned, document, "shift of {fifths} fifths");

        let spellings: Vec<String> = returned.tracks[0]
            .events
            .iter()
            .filter_map(|event| event.as_note())
            .map(|note| note.tpc.to_string())
            .collect();
        let expected: Vec<String> = document.tracks[0]
            .events
            .iter()
            .filter_map(|event| event.as_note())
            .map(|note| note.tpc.to_string())
            .collect();
        assert_eq!(spellings, expected, "shift of {fifths} fifths");
    }
}

/// Fixture C's double flat has to cross the boundary intact — it is the case
/// that breaks implementations which normalise pitch on the way out.
#[test]
fn a_double_flat_crosses_the_boundary() {
    let source = fixture_a();
    let by = Interval::between_tonics(source.tracks[0].tonic_tpc, Tpc::new(Letter::F, -1));
    let document = source.transpose(by);

    let xml = musicxml::to_string(&document).unwrap();
    // A double flat is an alter of -2 on a B, which is exactly how MusicXML
    // says it.
    assert!(
        xml.contains("<step>B</step><alter>-2</alter>"),
        "no double flat in the export"
    );
    // And it carries no accidental sign, because in F-flat major the double
    // flat is *in* the key signature. Writing one would be wrong.
    assert!(!xml.contains("<accidental>"), "{xml}");

    let returned = musicxml::from_str(&xml).unwrap();
    assert_eq!(returned, document);
    assert!(
        returned.tracks[0]
            .events
            .iter()
            .filter_map(|event| event.as_note())
            .any(|note| note.tpc == Tpc(-9))
    );
}

#[test]
fn the_export_says_what_other_programs_need() {
    let xml = musicxml::to_string(&fixture_a()).unwrap();
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<!DOCTYPE score-partwise"));
    assert!(xml.contains("<score-partwise version=\"4.0\">"));
    assert!(xml.contains("<work-title>Fixture A</work-title>"));
    assert!(xml.contains("<part-name>Melody</part-name>"));
    // Four flats, major.
    assert!(xml.contains("<fifths>-4</fifths><mode>major</mode>"));
    assert!(xml.contains("<beats>4</beats><beat-type>4</beat-type>"));
    assert!(xml.contains("<sound tempo=\"67\"/>"));
    // Eighths at two divisions per quarter are one division each.
    assert!(xml.contains("<divisions>2</divisions>"));
    assert!(xml.contains("<type>eighth</type>"));
    assert!(xml.contains("<rest/>"));
    // Two measures, no more.
    assert_eq!(xml.matches("<measure number=").count(), 2);
}

/// Notes in the key signature carry no accidental; only the ones that differ
/// from it do. Writing an accidental on every note is what makes a bad export
/// unreadable.
#[test]
fn accidentals_are_written_only_where_they_differ_from_the_key() {
    let plain = musicxml::to_string(&fixture_a()).unwrap();
    assert!(
        !plain.contains("<accidental>"),
        "Fixture A is entirely diatonic and needs no accidentals"
    );

    let chromatic = Document::new(
        "Chromatic",
        TimeSignature::COMMON,
        60,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                Note::new(Tpc::C, 4, ZERO, QUARTER).into(),
                Note::new(Tpc::new(Letter::F, 1), 4, QUARTER, QUARTER).into(),
                Note::new(Tpc::new(Letter::B, -1), 4, QUARTER * 2, QUARTER * 2).into(),
            ],
        )],
    );
    let xml = musicxml::to_string(&chromatic).unwrap();
    assert_eq!(xml.matches("<accidental>").count(), 2);
    assert!(xml.contains("<accidental>sharp</accidental>"));
    assert!(xml.contains("<accidental>flat</accidental>"));
    assert_eq!(musicxml::from_str(&xml).unwrap(), chromatic);
}

/// A note crossing a barline is two tied noteheads out there and one note here.
#[test]
fn a_tie_is_split_on_the_way_out_and_merged_on_the_way_back() {
    let document = Document::new(
        "Tied",
        TimeSignature::COMMON,
        60,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                Note::new(Tpc::C, 4, ZERO, QUARTER * 3).into(),
                // Starts on beat 4 and runs into the next bar.
                Note::new(Tpc::C, 4, QUARTER * 3, QUARTER * 2).into(),
                Note::new(Tpc::C, 4, QUARTER * 5, QUARTER * 3).into(),
            ],
        )],
    );
    let xml = musicxml::to_string(&document).unwrap();
    assert!(xml.contains("<tie type=\"start\"/>"), "{xml}");
    assert!(xml.contains("<tie type=\"stop\"/>"), "{xml}");
    assert!(xml.contains("<tied type=\"start\"/>"), "{xml}");

    let returned = musicxml::from_str(&xml).unwrap();
    assert_eq!(returned, document);
    // Three notes, not four: the tie was merged, not kept as two events.
    assert_eq!(returned.tracks[0].events.len(), 3);
}

#[test]
fn a_triplet_survives_with_its_arithmetic_intact() {
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
    let xml = musicxml::to_string(&document).unwrap();
    assert!(xml.contains("<divisions>3</divisions>"), "{xml}");
    assert!(
        xml.contains("<actual-notes>3</actual-notes><normal-notes>2</normal-notes>"),
        "{xml}"
    );
    assert_eq!(musicxml::from_str(&xml).unwrap(), document);
}

#[test]
fn several_parts_survive() {
    let document = Document::new(
        "Two parts",
        TimeSignature::COMMON,
        72,
        [
            Track::new(
                "Clarinet in B-flat",
                Tpc::new(Letter::B, -1),
                Mode::Major,
                [Note::new(Tpc::new(Letter::B, -1), 4, ZERO, QUARTER * 4).into()],
            ),
            Track::new(
                "Guitar",
                Tpc::new(Letter::D, 0),
                Mode::Dorian,
                [Note::new(Tpc::new(Letter::D, 0), 4, ZERO, QUARTER * 4).into()],
            ),
        ],
    );
    let xml = musicxml::to_string(&document).unwrap();
    assert!(xml.contains("<part id=\"P2\">"));
    // D dorian has the key signature of C major, not of D major.
    assert!(
        xml.contains("<fifths>0</fifths><mode>dorian</mode>"),
        "{xml}"
    );
    assert_eq!(musicxml::from_str(&xml).unwrap(), document);
}

/// A file from another program has none of our extensions, and must still read.
#[test]
fn a_foreign_file_reads() {
    let foreign = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="3.1">
  <part-list>
    <score-part id="P1"><part-name>Piano</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>-4</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
      </attributes>
      <note><pitch><step>A</step><alter>-1</alter><octave>4</octave></pitch>
        <duration>4</duration><type>whole</type></note>
    </measure>
  </part>
</score-partwise>
"#;
    let document = musicxml::from_str(foreign).unwrap();
    assert_eq!(document.tracks[0].name, "Piano");
    assert_eq!(document.time_sig, TimeSignature::COMMON);
    // Four flats with no mode given reads as A-flat major.
    assert_eq!(document.tracks[0].tonic_tpc.to_string(), "Ab");
    let note = document.tracks[0].events[0].as_note().unwrap();
    assert_eq!(note.tpc.to_string(), "Ab");
    assert_eq!(note.midi(), 68);
    assert_eq!(note.dur, QUARTER * 4);
    // No register was stashed, so one is chosen from the notes.
    assert_eq!(document.tracks[0].register, 4);
}

/// A note with no `alter` takes the key signature's spelling, which is what
/// every other program means by leaving it out.
#[test]
fn a_missing_alter_is_read_from_the_key() {
    let foreign = r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><key><fifths>-4</fifths></key>
      <time><beats>1</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration></note>
  </measure></part>
</score-partwise>"#;
    let document = musicxml::from_str(foreign).unwrap();
    let note = document.tracks[0].events[0].as_note().unwrap();
    // In four flats, an E with no alter is an E-flat.
    assert_eq!(note.tpc.to_string(), "Eb");
    assert_eq!(note.midi(), 63);
}

#[test]
fn nonsense_is_refused() {
    assert!(musicxml::from_str("not xml at all <<<").is_err());
    assert!(musicxml::from_str("<html><body/></html>").is_err());
    assert!(musicxml::from_str("<score-partwise version=\"4.0\"/>").is_err());
}
