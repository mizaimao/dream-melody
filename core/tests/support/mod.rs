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

//! The SPEC fixtures, built by hand so every later stage tests against the same
//! music.

#![allow(dead_code)]

use dream_melody_core::duration::{EIGHTH, QUARTER, ZERO};
use dream_melody_core::model::{Document, Event, Mode, Note, Rest, TimeSignature, Track};
use dream_melody_core::pitch::{Letter, Tpc};

/// A-flat, the tonic of Fixture A.
pub const A_FLAT: Tpc = Tpc::new(Letter::A, -1);
/// E-flat — degree 5 in A-flat major.
pub const E_FLAT: Tpc = Tpc::new(Letter::E, -1);
/// B-flat — degree 2 in A-flat major.
pub const B_FLAT: Tpc = Tpc::new(Letter::B, -1);
/// D-flat — degree 4 in A-flat major.
pub const D_FLAT: Tpc = Tpc::new(Letter::D, -1);

/// Fixture A, the reference melody. Tempo 67, 4/4, tonic A-flat.
///
/// ```text
/// | 0  5  1^ 5  1^ 5  1^ 2^ | 4^ 0  2^ 5  4^~4^ 2^ 5 |
/// ```
///
/// In ABC (see [`FIXTURE_A_ABC`]):
///
/// ```text
/// z E A E A E A B | d z B E d2 B E |]
/// ```
///
/// Unmarked degrees sit in the register A-flat 3 to G 4, so a bare `5` is
/// E-flat 4 and `1^` is A-flat 4.
///
/// It exercises rest handling, an off-beat entry, octave displacement, a tie,
/// and the G-sharp/A-flat spelling ambiguity.
pub fn fixture_a() -> Document {
    let eighth = |tpc: Tpc, octave: i32, step: i64| -> Event {
        Note::new(tpc, octave, EIGHTH * step, EIGHTH).into()
    };
    let rest = |step: i64| -> Event { Rest::new(EIGHTH * step, EIGHTH).into() };

    let events = [
        // | 0  5  1^ 5  1^ 5  1^ 2^ |
        rest(0),
        eighth(E_FLAT, 4, 1),
        eighth(A_FLAT, 4, 2),
        eighth(E_FLAT, 4, 3),
        eighth(A_FLAT, 4, 4),
        eighth(E_FLAT, 4, 5),
        eighth(A_FLAT, 4, 6),
        eighth(B_FLAT, 4, 7),
        // | 4^ 0  2^ 5  4^~4^ 2^ 5 |
        eighth(D_FLAT, 5, 8),
        rest(9),
        eighth(B_FLAT, 4, 10),
        eighth(E_FLAT, 4, 11),
        // The tie is not stored: two tied eighths are one quarter-long note.
        Note::new(D_FLAT, 5, EIGHTH * 12, QUARTER).into(),
        eighth(B_FLAT, 4, 14),
        eighth(E_FLAT, 4, 15),
    ];

    Document::new(
        "Fixture A",
        TimeSignature::COMMON,
        67,
        [Track::new("Melody", A_FLAT, Mode::Major, events)],
    )
}

/// Fixture A as ABC, the interchange form given in SPEC section 5.
pub const FIXTURE_A_ABC: &str = "\
X:1
M:4/4
L:1/8
Q:1/4=67
K:Ab
z E A E A E A B | d z B E d2 B E |]
";

/// MIDI numbers for Fixture A's sounding notes, in order.
///
/// E-flat 4 = 63, A-flat 4 = 68, B-flat 4 = 70, D-flat 5 = 73.
pub const FIXTURE_A_MIDI: [i32; 13] = [63, 68, 63, 68, 63, 68, 70, 73, 70, 63, 73, 70, 63];

/// Fixture A's onsets in eighth-note steps, paired with lengths in eighths.
pub const FIXTURE_A_GRID: [(i64, i64); 15] = [
    (0, 1),
    (1, 1),
    (2, 1),
    (3, 1),
    (4, 1),
    (5, 1),
    (6, 1),
    (7, 1),
    (8, 1),
    (9, 1),
    (10, 1),
    (11, 1),
    (12, 2),
    (14, 1),
    (15, 1),
];

/// Zero duration, re-exported so tests read cleanly.
pub const START: dream_melody_core::Duration = ZERO;

/// Fixture B — Fixture A transposed to B major, which must produce sharps.
///
/// A-flat to B is `+9` on the line of fifths. Used from stage 2.
pub const FIXTURE_B_SHIFT: i32 = 9;
/// The spellings Fixture B must produce, as `(degree, tpc)`.
pub const FIXTURE_B_SPELLINGS: [(u32, Tpc); 4] = [
    (1, Tpc::new(Letter::B, 0)),
    (2, Tpc::new(Letter::C, 1)),
    (4, Tpc::new(Letter::E, 0)),
    (5, Tpc::new(Letter::F, 1)),
];

/// Fixture C — Fixture A transposed to F-flat major, which must produce a
/// double flat.
///
/// A-flat to F-flat is `-4` on the line of fifths. Used from stage 2.
///
/// C-flat major, the original target in SPEC section 5, does not reach a double
/// accidental: every note in Fixture A is diatonic, and C-flat major spells all
/// seven of its degrees with single flats. F-flat major is the next key outward
/// and is where degree 4 becomes B-double-flat.
pub const FIXTURE_C_SHIFT: i32 = -4;
/// The spellings Fixture C must produce, as `(degree, tpc)`.
pub const FIXTURE_C_SPELLINGS: [(u32, Tpc); 4] = [
    (1, Tpc::new(Letter::F, -1)),
    (2, Tpc::new(Letter::G, -1)),
    (4, FIXTURE_C_DOUBLE_FLAT),
    (5, Tpc::new(Letter::C, -1)),
];
/// Degree 4 of F-flat major: B-double-flat, `Tpc(-9)`. The point of Fixture C.
pub const FIXTURE_C_DOUBLE_FLAT: Tpc = Tpc::new(Letter::B, -2);
