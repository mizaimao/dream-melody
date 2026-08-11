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

//! Stage 3 — Invariant 1, the lossless round trip.

// Tests may panic on purpose; that is how they report failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use dream_melody_core::duration::{EIGHTH, QUARTER, ZERO};
use dream_melody_core::interval::Interval;
use dream_melody_core::jianpu;
use dream_melody_core::model::{Document, Mode, Note, Rest, TimeSignature, Track};
use dream_melody_core::pitch::{Letter, Tpc};

use support::fixture_a;

/// Fixture A, written out. The SPEC gives this line informally; here it is with
/// the durations made explicit, which is what losslessness requires.
const FIXTURE_A_BODY: &str = "| 0_ 5_ 1^_ 5_ 1^_ 5_ 1^_ 2^_ | 4^_ 0_ 2^_ 5_ 4^ 2^_ 5_ |";

#[test]
fn fixture_a_writes_the_digits_the_spec_gives() {
    let text = jianpu::to_string(&fixture_a()).unwrap();
    assert!(text.contains(FIXTURE_A_BODY), "{text}");
    assert!(text.contains("1 = Ab3"), "{text}");
    assert!(text.contains("time: 4/4"), "{text}");
    assert!(text.contains("tempo: 67"), "{text}");
    assert!(text.contains("mode: Major"), "{text}");
}

/// Invariant 1. The round trip is lossless, for every key signature including
/// those requiring double accidentals.
#[test]
fn the_round_trip_is_lossless_in_every_key() {
    let source = fixture_a();
    // A shift of ±20 fifths reaches far past the practical keys, into
    // triple accidentals, which is exactly where a lossy format breaks.
    for fifths in -20..=20 {
        let document = source.transpose(Interval::simplest(fifths));
        let text = jianpu::to_string(&document).unwrap();
        let returned = jianpu::from_str(&text).unwrap();
        assert_eq!(returned, document, "shift of {fifths} fifths\n{text}");
        // And writing the returned document again gives the same text, so the
        // trip is stable rather than merely reversible once.
        assert_eq!(jianpu::to_string(&returned).unwrap(), text);
    }
}

/// The digits are what must not move. Transposition changes the header and
/// nothing else — that is the claim the whole model rests on.
#[test]
fn transposing_changes_the_key_line_and_leaves_the_digits_alone() {
    let source = fixture_a();
    let bodies: Vec<String> = (-20..=20)
        .map(|fifths| {
            let text = jianpu::to_string(&source.transpose(Interval::simplest(fifths))).unwrap();
            text.lines()
                .find(|line| line.starts_with('|'))
                .unwrap()
                .to_string()
        })
        .collect();

    for body in &bodies {
        assert_eq!(body, FIXTURE_A_BODY, "the digits moved under transposition");
    }
}

/// Double accidentals survive the round trip, in the notation as well as the
/// model. F-flat major is Fixture C's key.
#[test]
fn a_double_flat_survives_the_round_trip() {
    let source = fixture_a();
    let by = Interval::between_tonics(source.tracks[0].tonic_tpc, Tpc::new(Letter::F, -1));
    let document = source.transpose(by);

    let text = jianpu::to_string(&document).unwrap();
    assert!(text.contains("1 = Fb3"), "{text}");
    // The digits are unchanged: a double flat in the key signature is the
    // header's problem, not the melody's.
    assert!(text.contains(FIXTURE_A_BODY), "{text}");

    let returned = jianpu::from_str(&text).unwrap();
    assert_eq!(returned, document);
    let spellings: Vec<String> = returned.tracks[0]
        .events
        .iter()
        .filter_map(|event| event.as_note())
        .map(|note| note.tpc.to_string())
        .collect();
    assert!(spellings.contains(&"Bbb".to_string()), "{spellings:?}");
}

/// An accidental outside the key keeps its digit and gains a sign, which is how
/// numbered notation writes a chromatic note.
#[test]
fn a_chromatic_note_keeps_its_digit() {
    let document = Document::new(
        "Chromatic",
        TimeSignature::COMMON,
        60,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                // C D D# E in C major: the third note is a raised second.
                Note::new(Tpc::C, 4, ZERO, QUARTER).into(),
                Note::new(Tpc::new(Letter::D, 0), 4, QUARTER, QUARTER).into(),
                Note::new(Tpc::new(Letter::D, 1), 4, QUARTER * 2, QUARTER).into(),
                Note::new(Tpc::new(Letter::E, 0), 4, QUARTER * 3, QUARTER).into(),
            ],
        )],
    );
    let text = jianpu::to_string(&document).unwrap();
    assert!(text.contains("| 1 2 #2 3 |"), "{text}");
    assert_eq!(jianpu::from_str(&text).unwrap(), document);
}

/// A triplet has no beam-and-dot spelling, so it is written exactly rather than
/// rounded to something that would not sum.
#[test]
fn a_triplet_round_trips_exactly() {
    // A triplet eighth: three of them in the time of one quarter.
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
    let text = jianpu::to_string(&document).unwrap();
    assert!(text.contains("[1/12]"), "{text}");

    let returned = jianpu::from_str(&text).unwrap();
    assert_eq!(returned, document);
    // The three still sum to exactly one beat. No float ever touched them.
    let total: dream_melody_core::Duration = returned.tracks[0]
        .events
        .iter()
        .map(|event| event.dur())
        .sum();
    assert_eq!(total, QUARTER);
    assert_eq!(returned.validate(), []);
}

/// Octave marks read in both directions.
#[test]
fn octave_marks_go_up_and_down() {
    let document = Document::new(
        "Octaves",
        TimeSignature::COMMON,
        60,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                Note::new(Tpc::C, 3, ZERO, QUARTER).into(),
                Note::new(Tpc::C, 4, QUARTER, QUARTER).into(),
                Note::new(Tpc::C, 5, QUARTER * 2, QUARTER).into(),
                Note::new(Tpc::C, 6, QUARTER * 3, QUARTER).into(),
            ],
        )],
    );
    let text = jianpu::to_string(&document).unwrap();
    // The register is chosen from the lowest note, so nothing needs a mark
    // below the line.
    assert!(text.contains("1 = C3"), "{text}");
    assert!(text.contains("| 1 1^ 1^^ 1^^^ |"), "{text}");
    assert_eq!(jianpu::from_str(&text).unwrap(), document);
}

#[test]
fn a_rest_round_trips_with_its_length() {
    let document = Document::new(
        "Rests",
        TimeSignature::COMMON,
        60,
        [Track::new(
            "Melody",
            Tpc::C,
            Mode::Major,
            [
                Rest::new(ZERO, EIGHTH).into(),
                Note::new(Tpc::C, 4, EIGHTH, EIGHTH).into(),
                Rest::new(QUARTER, QUARTER * 3).into(),
            ],
        )],
    );
    let text = jianpu::to_string(&document).unwrap();
    assert!(text.contains("| 0_ 1_ 0 - - |"), "{text}");
    assert_eq!(jianpu::from_str(&text).unwrap(), document);
}

#[test]
fn several_tracks_round_trip_together() {
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
                "Guitar, capo 1",
                Tpc::new(Letter::C, 0),
                Mode::Mixolydian,
                [Note::new(Tpc::C, 4, ZERO, QUARTER * 4).into()],
            ),
        ],
    );
    let text = jianpu::to_string(&document).unwrap();
    assert!(text.contains("track: Clarinet in B-flat"), "{text}");
    assert!(text.contains("1 = Bb4"), "{text}");
    assert!(text.contains("mode: Mixolydian"), "{text}");
    assert_eq!(jianpu::from_str(&text).unwrap(), document);
}

#[test]
fn nonsense_is_refused_with_a_line_number() {
    let bad = "\
title: T
time: 4/4
tempo: 60

track: Melody
1 = C4
mode: Major
| 1 9 |
";
    let error = jianpu::from_str(bad).unwrap_err();
    assert!(error.to_string().starts_with("line 8:"), "{error}");
    assert!(error.to_string().contains("degree"), "{error}");
}
