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

//! JSON persistence, against the real fixtures rather than a toy document.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::interval::Interval;
use dream_melody_core::json;
use dream_melody_core::pitch::{Letter, Tpc};

use support::fixture_a;

#[test]
fn fixture_a_survives_a_save_and_load() {
    let document = fixture_a();
    let loaded = json::from_str(&json::to_string(&document).unwrap()).unwrap();
    assert_eq!(loaded, document);
    // Including the tie, which is one note of two eighths and must not become
    // two notes on the way through.
    assert_eq!(loaded.tracks[0].events.len(), 15);
}

/// A double flat has to survive persistence, or Fixture C proves nothing about
/// the parts of the program a user actually touches.
#[test]
fn a_double_flat_survives_a_save_and_load() {
    let f_flat_major = Tpc::new(Letter::F, -1);
    let source = fixture_a();
    let by = Interval::between_tonics(source.tracks[0].tonic_tpc, f_flat_major);
    let transposed = source.transpose(by);

    let json = json::to_string(&transposed).unwrap();
    // Tpc(-9) is B-double-flat, written as a bare integer.
    assert!(json.contains("\"tpc\":-9"), "{json}");

    let loaded = json::from_str(&json).unwrap();
    assert_eq!(loaded, transposed);
    let spellings: Vec<String> = loaded.tracks[0]
        .events
        .iter()
        .filter_map(|event| event.as_note())
        .map(|note| note.tpc.to_string())
        .collect();
    assert!(spellings.contains(&"Bbb".to_string()), "{spellings:?}");
}

/// The file is JSON, and it is meant to be legible when opened by hand.
#[test]
fn the_saved_file_reads_the_way_a_person_would_expect() {
    let pretty = json::to_string_pretty(&fixture_a()).unwrap();
    assert!(pretty.starts_with("{\n  \"format\": 1,"), "{pretty}");
    assert!(pretty.contains("\"title\": \"Fixture A\""));
    assert!(pretty.contains("\"bpm\": 67"));
    assert!(pretty.contains("\"mode\": \"Major\""));
    // Durations read as fractions, not as pairs of numbers.
    assert!(pretty.contains("\"dur\": \"1/8\""), "{pretty}");
    assert!(pretty.contains("\"onset\": \"3/2\""), "{pretty}");
    // The tie: one note of a quarter, not two eighths.
    assert!(pretty.contains("\"dur\": \"1/4\""), "{pretty}");
    // No floats anywhere. Rhythm is exact or it is nothing.
    assert!(!pretty.contains('.'), "{pretty}");
}
