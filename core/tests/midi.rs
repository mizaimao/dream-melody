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

//! Stage 4 — Invariant 4, and the honest limits of MIDI.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::duration::{QUARTER, ZERO};
use dream_melody_core::interval::Interval;
use dream_melody_core::midi;
use dream_melody_core::model::{Document, Event, Mode, Note, TimeSignature, Track};
use dream_melody_core::pitch::{Letter, Tpc};

use support::{FIXTURE_A_MIDI, fixture_a};

fn notes(document: &Document) -> Vec<Note> {
    document.tracks[0]
        .events
        .iter()
        .filter_map(Event::as_note)
        .copied()
        .collect()
}

/// Invariant 4. Rational arithmetic in, no float drift out.
#[test]
fn onsets_and_durations_round_trip_exactly() {
    let document = fixture_a();
    let bytes = midi::to_bytes(&document).unwrap();
    let returned = midi::from_bytes(&bytes).unwrap();

    let before = notes(&document);
    let after = notes(&returned);
    assert_eq!(before.len(), after.len());
    for (before, after) in before.iter().zip(&after) {
        assert_eq!(after.onset, before.onset, "onset drifted");
        assert_eq!(after.dur, before.dur, "duration drifted");
        assert_eq!(after.midi(), before.midi(), "pitch changed");
    }

    // The tie is still one note of a quarter, not two eighths.
    assert_eq!(after[10].dur, QUARTER);
    assert_eq!(returned.time_sig, document.time_sig);
    assert_eq!(returned.bpm, document.bpm);
    assert_eq!(returned.title, document.title);
}

/// A tempo that does not divide evenly into a microsecond count is the one
/// value MIDI rounds. It has to survive anyway.
#[test]
fn awkward_tempi_come_back_unchanged() {
    for bpm in [40, 67, 89, 120, 137, 208] {
        let mut document = fixture_a();
        document.bpm = bpm;
        let returned = midi::from_bytes(&midi::to_bytes(&document).unwrap()).unwrap();
        assert_eq!(returned.bpm, bpm, "tempo {bpm} drifted");
    }
}

/// Triplets are where a file with too coarse a resolution silently rounds.
#[test]
fn triplets_land_on_whole_ticks() {
    let third = QUARTER / 3;
    let document = Document::new(
        "Triplet",
        TimeSignature::COMMON,
        120,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                Note::new(Tpc::C, 4, ZERO, third).into(),
                Note::new(Tpc::C, 4, third, third).into(),
                Note::new(Tpc::C, 4, third * 2, third).into(),
                Note::new(Tpc::C, 4, QUARTER, QUARTER * 3).into(),
            ],
        )],
    );
    let returned = midi::from_bytes(&midi::to_bytes(&document).unwrap()).unwrap();
    let after = notes(&returned);
    for (index, note) in after.iter().enumerate().take(3) {
        assert_eq!(note.dur, third, "triplet {index} drifted");
    }
    // The three still sum to exactly one quarter, which is the whole point.
    assert_eq!(after[0].dur + after[1].dur + after[2].dur, QUARTER);
}

/// Fixture A stays in its key, so its spelling survives even MIDI: the key
/// signature is enough to put every note back where it was.
#[test]
fn diatonic_spelling_is_recovered_from_the_key_signature() {
    let document = fixture_a();
    let returned = midi::from_bytes(&midi::to_bytes(&document).unwrap()).unwrap();

    assert_eq!(returned.tracks[0].tonic_tpc.to_string(), "Ab");
    let spellings: Vec<String> = notes(&returned)
        .iter()
        .map(|note| note.tpc.to_string())
        .collect();
    let expected: Vec<String> = notes(&document)
        .iter()
        .map(|note| note.tpc.to_string())
        .collect();
    assert_eq!(spellings, expected);
    assert_eq!(
        notes(&returned)
            .iter()
            .map(Note::midi)
            .collect::<Vec<i32>>(),
        FIXTURE_A_MIDI
    );
}

/// The limit, stated as a test so nobody is surprised by it later. MIDI has one
/// byte per pitch, so an enharmonic spelling cannot survive and does not.
#[test]
fn midi_cannot_keep_an_enharmonic_spelling() {
    let document = Document::new(
        "B sharp",
        TimeSignature::COMMON,
        120,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [Note::new(Tpc::new(Letter::B, 1), 4, ZERO, QUARTER * 4).into()],
        )],
    );
    let returned = midi::from_bytes(&midi::to_bytes(&document).unwrap()).unwrap();
    let note = notes(&returned)[0];

    // The sound is right to the byte...
    assert_eq!(note.midi(), 72);
    // ...and the spelling is gone: B-sharp 4 comes back as C 5, because in a
    // MIDI file those are the same thing. This is why MusicXML exists.
    assert_eq!(note.tpc.to_string(), "C");
    assert_eq!(note.octave, 5);
    assert_ne!(note.tpc, Tpc::new(Letter::B, 1));
}

/// A key too far out for MIDI's seven-flat limit must not be written wrong.
#[test]
fn a_key_midi_cannot_express_is_left_out_rather_than_corrupted() {
    let source = fixture_a();
    let by = Interval::between_tonics(source.tracks[0].tonic_tpc, Tpc::new(Letter::F, -1));
    let document = source.transpose(by);
    assert_eq!(document.tracks[0].key_fifths(), -8);

    let bytes = midi::to_bytes(&document).unwrap();
    // No key signature meta event at all, rather than a clamped, wrong one.
    assert!(
        !bytes
            .windows(3)
            .any(|window| window[0] == 0xff && window[1] == 0x59),
        "a key signature MIDI cannot hold was written anyway"
    );

    // Timing still round-trips exactly; only the spelling is lost.
    let returned = midi::from_bytes(&bytes).unwrap();
    for (before, after) in notes(&document).iter().zip(&notes(&returned)) {
        assert_eq!(after.onset, before.onset);
        assert_eq!(after.dur, before.dur);
        assert_eq!(after.midi(), before.midi());
    }
}

#[test]
fn the_file_is_a_standard_midi_file() {
    let bytes = midi::to_bytes(&fixture_a()).unwrap();
    assert_eq!(&bytes[0..4], b"MThd");
    assert_eq!(&bytes[4..8], &[0, 0, 0, 6]);
    // Format 1, two tracks: a conductor and the melody.
    assert_eq!(&bytes[8..10], &[0, 1]);
    assert_eq!(&bytes[10..12], &[0, 2]);
    assert_eq!(
        bytes.windows(4).filter(|window| *window == b"MTrk").count(),
        2
    );
    // Every chunk ends with an end-of-track meta event.
    assert_eq!(
        bytes
            .windows(3)
            .filter(|window| *window == [0xff, 0x2f, 0x00])
            .count(),
        2
    );
}

#[test]
fn several_tracks_survive() {
    let document = Document::new(
        "Two parts",
        TimeSignature::COMMON,
        90,
        [
            Track::new(
                "One",
                Tpc::C,
                Mode::Major,
                [Note::new(Tpc::C, 4, ZERO, QUARTER * 4).into()],
            ),
            Track::new(
                "Two",
                Tpc::C,
                Mode::Major,
                [Note::new(Tpc::C, 5, ZERO, QUARTER * 4).into()],
            ),
        ],
    );
    let returned = midi::from_bytes(&midi::to_bytes(&document).unwrap()).unwrap();
    assert_eq!(returned.tracks.len(), 2);
    assert_eq!(returned.tracks[0].name, "One");
    assert_eq!(returned.tracks[1].name, "Two");
    assert_eq!(returned.tracks[1].events[0].as_note().unwrap().midi(), 72);
}

#[test]
fn rubbish_is_refused() {
    assert!(midi::from_bytes(b"").is_err());
    assert!(midi::from_bytes(b"not a midi file at all").is_err());
    assert!(midi::from_bytes(b"MThd\0\0\0\x06\0\x01\0\x01\x80\x01").is_err());
}
