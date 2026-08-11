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

//! Stage 2 — Invariant 2, and Fixtures B and C.
//!
//! Transposition must preserve spelling correctness. These tests assert the
//! spellings, not just the sounding pitches, because every naive implementation
//! gets the pitches right and the spellings wrong.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::interval::Interval;
use dream_melody_core::model::{Document, Note, Track};
use dream_melody_core::pitch::{Letter, Tpc};

use support::{FIXTURE_A_MIDI, FIXTURE_B_SPELLINGS, FIXTURE_C_SPELLINGS, fixture_a};

/// B major — the target of Fixture B.
const B_MAJOR: Tpc = Tpc::new(Letter::B, 0);
/// F-flat major — the target of Fixture C, and the nearest key to A-flat that
/// forces a double accidental.
const F_FLAT_MAJOR: Tpc = Tpc::new(Letter::F, -1);

fn spellings(track: &Track) -> Vec<String> {
    track
        .events
        .iter()
        .filter_map(|event| event.as_note())
        .map(|note| format!("{}{}", note.tpc, note.octave))
        .collect()
}

fn midi(track: &Track) -> Vec<i32> {
    track
        .events
        .iter()
        .filter_map(|event| event.as_note())
        .map(Note::midi)
        .collect()
}

fn only_track(document: &Document) -> &Track {
    &document.tracks[0]
}

/// Fixture B — Fixture A transposed to B major. Must produce sharps.
#[test]
fn fixture_b_is_b_major_in_sharps() {
    let source = fixture_a();
    let by = Interval::between_tonics(only_track(&source).tonic_tpc, B_MAJOR);
    // A-flat to B is an augmented second up: three semitones.
    assert_eq!(by.semitones(), 3);

    let transposed = source.transpose(by);
    let track = only_track(&transposed);
    assert_eq!(track.tonic_tpc, B_MAJOR);

    for note in track.events.iter().filter_map(|event| event.as_note()) {
        assert!(
            note.tpc.accidental() >= 0,
            "{} is spelled with a flat in a sharp key",
            note.tpc
        );
    }

    // The four degrees land exactly where the fixture data says.
    let expected: Vec<String> = FIXTURE_B_SPELLINGS
        .iter()
        .map(|(_, tpc)| tpc.to_string())
        .collect();
    for spelled in spellings(track) {
        let letter_and_accidental = spelled.trim_end_matches(char::is_numeric).to_string();
        assert!(
            expected.contains(&letter_and_accidental),
            "unexpected spelling {spelled}"
        );
    }

    // Octaves are derived, not guessed: B-flat 4 becomes C-sharp 5, crossing
    // the octave boundary, while the notes either side of it do not.
    assert_eq!(
        spellings(track),
        [
            "F#4", "B4", "F#4", "B4", "F#4", "B4", "C#5", "E5", "C#5", "F#4", "E5", "C#5", "F#4"
        ]
    );

    // And it still sounds three semitones higher than it did.
    let expected_midi: Vec<i32> = FIXTURE_A_MIDI.iter().map(|pitch| pitch + 3).collect();
    assert_eq!(midi(track), expected_midi);

    // The music itself is untouched: same rhythm, same bars.
    assert_eq!(transposed.validate(), []);
}

/// Fixture C — Fixture A transposed to F-flat major. Must produce a double flat
/// and must not panic.
#[test]
fn fixture_c_reaches_a_double_flat_without_panicking() {
    let source = fixture_a();
    let by = Interval::between_tonics(only_track(&source).tonic_tpc, F_FLAT_MAJOR);
    // A-flat to F-flat is a major third down: four semitones.
    assert_eq!(by.semitones(), -4);

    let transposed = source.transpose(by);
    let track = only_track(&transposed);
    assert_eq!(track.tonic_tpc, F_FLAT_MAJOR);
    assert_eq!(track.tonic_tpc.to_string(), "Fb");

    assert_eq!(
        spellings(track),
        [
            "Cb4", "Fb4", "Cb4", "Fb4", "Cb4", "Fb4", "Gb4", "Bbb4", "Gb4", "Cb4", "Bbb4", "Gb4",
            "Cb4"
        ]
    );

    // The point of the fixture: degree 4 is B-double-flat.
    let double_flats: Vec<&Note> = track
        .events
        .iter()
        .filter_map(|event| event.as_note())
        .filter(|note| note.tpc.accidental_depth() > 0)
        .collect();
    assert_eq!(
        double_flats.len(),
        2,
        "both degree-4 notes are double flats"
    );
    assert_eq!(double_flats[0].tpc.to_string(), "Bbb");
    assert_eq!(double_flats[0].tpc, Tpc(-9));

    // The model does not clamp. Only a renderer may.
    assert_eq!(double_flats[0].tpc.accidental(), -2);

    let expected_midi: Vec<i32> = FIXTURE_A_MIDI.iter().map(|pitch| pitch - 4).collect();
    assert_eq!(midi(track), expected_midi);
    assert_eq!(transposed.validate(), []);

    // Same spellings the fixture data promised, one octave-crossing included:
    // D-flat 5 becomes B-double-flat 4.
    let expected: Vec<String> = FIXTURE_C_SPELLINGS
        .iter()
        .map(|(_, tpc)| tpc.to_string())
        .collect();
    assert_eq!(expected, ["Fb", "Gb", "Bbb", "Cb"]);
}

/// Transposition is a relabelling. Scale degrees — the distance from the tonic
/// along the line of fifths — must come out identical, which is the entire
/// reason the model stores what it stores.
#[test]
fn scale_degrees_survive_any_transposition() {
    let source = fixture_a();
    let degrees = |track: &Track| -> Vec<i32> {
        track
            .events
            .iter()
            .filter_map(|event| event.as_note())
            .map(|note| note.tpc.0 - track.tonic_tpc.0)
            .collect()
    };

    for fifths in -30..=30 {
        let by = Interval::simplest(fifths);
        let moved = source.transpose(by);
        assert_eq!(
            degrees(only_track(&moved)),
            degrees(only_track(&source)),
            "degrees changed under a shift of {fifths}"
        );
    }
}

/// Transposing there and back is identity, including the octaves.
#[test]
fn transposition_round_trips() {
    let source = fixture_a();
    for fifths in -30..=30 {
        for octaves in -2..=2 {
            let by = Interval::simplest(fifths).with_octaves(octaves);
            let there_and_back = source.transpose(by).transpose(by.inverted());
            assert_eq!(there_and_back, source, "shift {fifths}, octaves {octaves}");
        }
    }
}

/// The octave-derivation path, isolated. B-sharp 4 is the note that breaks
/// implementations that fold to a pitch class first.
#[test]
fn octaves_follow_letters_through_a_transposition() {
    use dream_melody_core::duration::{WHOLE, ZERO};

    let b_sharp_4 = Note::new(Tpc::new(Letter::B, 1), 4, ZERO, WHOLE);
    assert_eq!(b_sharp_4.midi(), 72);

    // Up a diminished second lands on C-sharp 5 — one letter up, one octave up,
    // and no change in sound at all.
    let diminished_second_up = Interval::new(1, -5).expect("a second is five fifths flatward");
    let moved = b_sharp_4.transpose(diminished_second_up);
    assert_eq!(moved.tpc.to_string(), "C#");
    assert_eq!(moved.octave, 5);
    assert_eq!(moved.midi(), 73);

    // Down a semitone spelled as a letter change: B-sharp 4 to B 4.
    let augmented_unison_down = Interval::new(0, 7).expect("an augmented unison").inverted();
    let lowered = b_sharp_4.transpose(augmented_unison_down);
    assert_eq!(lowered.tpc.to_string(), "B");
    assert_eq!(lowered.octave, 4);
    assert_eq!(lowered.midi(), 71);
}

/// Invariant 2, stated as the SPEC states it, through the document API.
#[test]
fn up_a_minor_second_from_c_major_gives_d_flat_major() {
    let by = Interval::by_semitones(1, Tpc::C);
    assert_eq!(Tpc::C.transpose(by).to_string(), "Db");
    assert_eq!(by.semitones(), 1);

    // C-sharp major is the other spelling of the same movement, and it is the
    // one with more accidentals, so it is not what you get by default.
    let deliberate_sharps = Interval::new(0, 7).expect("an augmented unison");
    assert_eq!(deliberate_sharps.semitones(), 1);
    assert_eq!(Tpc::C.transpose(deliberate_sharps).to_string(), "C#");
}
