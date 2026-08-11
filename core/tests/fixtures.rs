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

//! Acceptance tests for the SPEC fixtures that stage 1 can reach.
//!
//! Fixture D is the whole point of this stage. Fixture A is checked for
//! internal consistency so that later stages can trust it; Fixtures B and C are
//! transpositions and belong to stage 2.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::Duration;
use dream_melody_core::duration::{EIGHTH, WHOLE, ZERO};
use dream_melody_core::model::{Note, TimeSignature};
use dream_melody_core::pitch::{Letter, Tpc};

use support::{
    A_FLAT, B_FLAT, D_FLAT, E_FLAT, FIXTURE_A_GRID, FIXTURE_A_MIDI, FIXTURE_B_SHIFT,
    FIXTURE_B_SPELLINGS, FIXTURE_C_DOUBLE_FLAT, FIXTURE_C_SHIFT, FIXTURE_C_SPELLINGS, fixture_a,
};

/// Fixture D — the octave bug. A single B-sharp 4 must report MIDI 72.
///
/// Octave numbering follows the letter, not the sounding pitch. B-sharp 4 is
/// written on the B line of octave 4 and sounds a semitone above it, which is
/// C5's pitch: MIDI 72, not 60 and not 61.
#[test]
fn fixture_d_b_sharp_4_is_midi_72() {
    let b_sharp_4 = Note::new(Tpc::new(Letter::B, 1), 4, ZERO, WHOLE);
    assert_eq!(b_sharp_4.midi(), 72);
}

/// The mirror of Fixture D, which fails the same way for the same reason.
#[test]
fn c_flat_4_is_midi_59() {
    let c_flat_4 = Note::new(Tpc::new(Letter::C, -1), 4, ZERO, WHOLE);
    assert_eq!(c_flat_4.midi(), 59);
    // And the naturals either side, so the test cannot pass by luck.
    assert_eq!(
        Note::new(Tpc::natural(Letter::C), 4, ZERO, WHOLE).midi(),
        60
    );
    assert_eq!(
        Note::new(Tpc::natural(Letter::B), 4, ZERO, WHOLE).midi(),
        71
    );
    assert_eq!(
        Note::new(Tpc::natural(Letter::B), 3, ZERO, WHOLE).midi(),
        59
    );
}

#[test]
fn fixture_a_fills_exactly_two_bars() {
    let document = fixture_a();
    assert_eq!(document.validate(), []);
    assert_eq!(document.time_sig, TimeSignature::COMMON);
    assert_eq!(document.bpm, 67);
    assert_eq!(document.tracks[0].length(), WHOLE * 2);
}

#[test]
fn fixture_a_sits_on_the_eighth_note_grid() {
    let track = &fixture_a().tracks[0];
    let grid: Vec<(i64, i64)> = track
        .events
        .iter()
        .map(|event| {
            let steps = |value: Duration| (value / EIGHTH).to_integer();
            (steps(event.onset()), steps(event.dur()))
        })
        .collect();
    assert_eq!(grid, FIXTURE_A_GRID);
    // The tie: two notated eighths stored as one note of two eighths.
    assert_eq!(grid[12], (12, 2));
}

#[test]
fn fixture_a_sounds_the_expected_pitches() {
    let track = &fixture_a().tracks[0];
    let midi: Vec<i32> = track
        .events
        .iter()
        .filter_map(|event| event.as_note())
        .map(Note::midi)
        .collect();
    assert_eq!(midi, FIXTURE_A_MIDI);
}

/// Fixture A's four degrees, as `(degree, tpc)` in its home key of A-flat.
const FIXTURE_A_SPELLINGS: [(u32, Tpc); 4] = [(1, A_FLAT), (2, B_FLAT), (4, D_FLAT), (5, E_FLAT)];

/// Checks the transposition targets before stage 2 relies on them.
///
/// Shifting the line of fifths is arithmetic, not yet the transposition API, so
/// this validates the fixture data rather than the feature.
#[test]
fn fixture_b_and_c_targets_land_where_the_spec_says() {
    let shifted = |shift: i32| -> Vec<(u32, Tpc)> {
        FIXTURE_A_SPELLINGS
            .iter()
            .map(|&(degree, tpc)| (degree, Tpc(tpc.0 + shift)))
            .collect()
    };

    // Fixture B — B major, all sharps and naturals, no flat anywhere.
    assert_eq!(shifted(FIXTURE_B_SHIFT), FIXTURE_B_SPELLINGS);
    assert!(
        FIXTURE_B_SPELLINGS
            .iter()
            .all(|(_, tpc)| tpc.accidental() >= 0)
    );

    // Fixture C — F-flat major, and degree 4 is the double flat it exists for.
    assert_eq!(shifted(FIXTURE_C_SHIFT), FIXTURE_C_SPELLINGS);
    assert_eq!(FIXTURE_C_DOUBLE_FLAT.to_string(), "Bbb");
    assert_eq!(FIXTURE_C_DOUBLE_FLAT, Tpc(-9));
    assert!(
        FIXTURE_C_SPELLINGS
            .iter()
            .any(|(_, tpc)| tpc.accidental_depth() > 0),
        "Fixture C must reach a double accidental"
    );
}

/// Why Fixture C moved off C-flat major: that key cannot reach a double flat
/// with a melody as diatonic as Fixture A.
#[test]
fn c_flat_major_would_have_tested_nothing() {
    let c_flat_major: Vec<Tpc> = FIXTURE_A_SPELLINGS
        .iter()
        .map(|&(_, tpc)| Tpc(tpc.0 - 3))
        .collect();
    assert_eq!(
        c_flat_major
            .iter()
            .map(Tpc::to_string)
            .collect::<Vec<String>>(),
        ["Cb", "Db", "Fb", "Gb"]
    );
    assert!(
        c_flat_major.iter().all(|tpc| tpc.accidental_depth() == 0),
        "every degree is a single flat, so no double accidental is exercised"
    );
}

/// The spelling ambiguity the fixture exists to pin down.
#[test]
fn fixture_a_is_spelled_in_flats_not_sharps() {
    let track = &fixture_a().tracks[0];
    assert_eq!(track.tonic_tpc.to_string(), "Ab");
    for note in track.events.iter().filter_map(|event| event.as_note()) {
        assert!(
            note.tpc.accidental() <= 0,
            "{} is spelled with a sharp in a flat key",
            note.tpc
        );
    }
    // A-flat and G-sharp sound alike and must not be equal in the model.
    assert_ne!(track.tonic_tpc, Tpc::new(Letter::G, 1));
    assert_eq!(
        track.tonic_tpc.pitch_class(),
        Tpc::new(Letter::G, 1).pitch_class()
    );
}
