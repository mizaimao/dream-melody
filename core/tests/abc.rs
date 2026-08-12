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

//! ABC export, checked against the ABC the SPEC itself gives for Fixture A.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::abc;
use dream_melody_core::duration::{QUARTER, ZERO};
use dream_melody_core::interval::Interval;
use dream_melody_core::model::{Document, Mode, Note, TimeSignature, Track};
use dream_melody_core::pitch::{Letter, Tpc};

use support::{FIXTURE_A_ABC, fixture_a};

/// The SPEC gives Fixture A in ABC. The export must produce that, not merely
/// something equivalent.
#[test]
fn fixture_a_matches_the_abc_the_spec_gives() {
    let written = abc::to_string(&fixture_a()).unwrap();

    // The SPEC's listing has no title line; compare the rest field for field.
    let expected: Vec<&str> = FIXTURE_A_ABC.trim().lines().collect();
    let actual: Vec<&str> = written
        .trim()
        .lines()
        .filter(|line| !line.starts_with("T:"))
        .collect();

    assert_eq!(actual[0], expected[0], "X: line");
    assert_eq!(actual[1], expected[1], "M: line");
    assert_eq!(actual[2], expected[2], "L: line");
    assert_eq!(actual[3], expected[3], "Q: line");
    assert_eq!(actual[4], expected[4], "K: line");
    assert_eq!(actual[5], expected[5], "the tune itself");
}

#[test]
fn the_header_says_what_abc_needs() {
    let written = abc::to_string(&fixture_a()).unwrap();
    assert!(written.contains("X:1\n"));
    assert!(written.contains("T:Fixture A\n"));
    assert!(written.contains("M:4/4\n"));
    assert!(written.contains("L:1/8\n"));
    assert!(written.contains("Q:1/4=67\n"));
    assert!(written.contains("K:Ab\n"));
}

/// Transposing changes only the key line and the letters, never the shape.
#[test]
fn a_sharp_key_writes_sharps() {
    let source = fixture_a();
    let by = Interval::between_tonics(source.tracks[0].tonic_tpc, Tpc::new(Letter::B, 0));
    let written = abc::to_string(&source.transpose(by)).unwrap();
    assert!(written.contains("K:B\n"), "{written}");
    // Every note is diatonic in B major, so no accidental is written at all.
    assert!(!written.contains('^'), "{written}");
    assert!(!written.contains('_'), "{written}");
    // F-sharp 4, B 4, C-sharp 5, E 5 — the sharps live in the K: line.
    assert!(
        written.contains("z F B F B F B c | e z c F e2 c F |]"),
        "{written}"
    );
}

/// A double flat is two underscores, and only when the key does not already
/// imply it.
#[test]
fn a_double_flat_writes_as_two_underscores() {
    let document = Document::new(
        "Double flat",
        TimeSignature::COMMON,
        60,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                Note::new(Tpc::C, 4, ZERO, QUARTER * 2).into(),
                Note::new(Tpc::new(Letter::B, -2), 4, QUARTER * 2, QUARTER * 2).into(),
            ],
        )],
    );
    let written = abc::to_string(&document).unwrap();
    assert!(written.contains("__B"), "{written}");
}

/// A note crossing a barline is a hyphen and a repeat, which is ABC's tie.
#[test]
fn a_tie_crosses_the_barline_with_a_hyphen() {
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
                Note::new(Tpc::C, 4, QUARTER * 3, QUARTER * 2).into(),
                Note::new(Tpc::C, 4, QUARTER * 5, QUARTER * 3).into(),
            ],
        )],
    );
    let written = abc::to_string(&document).unwrap();
    assert!(written.contains("C3 C- | C C3 |]"), "{written}");
}

#[test]
fn each_track_becomes_its_own_tune() {
    let document = Document::new(
        "Two parts",
        TimeSignature::COMMON,
        72,
        [
            Track::new(
                "One",
                Tpc::C,
                Mode::Major,
                [Note::new(Tpc::C, 4, ZERO, QUARTER * 4).into()],
            ),
            Track::new(
                "Two",
                Tpc::new(Letter::G, 0),
                Mode::Major,
                [Note::new(Tpc::new(Letter::G, 0), 4, ZERO, QUARTER * 4).into()],
            ),
        ],
    );
    let written = abc::to_string(&document).unwrap();
    assert!(written.contains("X:1"));
    assert!(written.contains("X:2"));
    assert!(written.contains("K:C\n"));
    assert!(written.contains("K:G\n"));
}
