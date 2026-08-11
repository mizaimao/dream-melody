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

//! Model and music-theory core for Dream Melody.
//!
//! Pitch is stored as a signed position on the line of fifths (a *tonal pitch
//! class*, or TPC) rather than as a MIDI number, so G-sharp and A-flat are
//! distinct values. Durations are exact rationals. Every rendering — staff,
//! numbered notation (jianpu/简谱), ABC, MIDI — is a pure function of this
//! model.
//!
//! This crate has zero UI dependencies.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod duration;
pub mod interval;
pub mod json;
pub mod model;
pub mod pitch;

pub use duration::Duration;
pub use interval::Interval;
pub use model::{Document, Event, Mode, Note, Rest, TimeSignature, Track};
pub use pitch::{Letter, Tpc};
