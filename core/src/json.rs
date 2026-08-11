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

//! Persistence. A saved score is JSON, not a bespoke format.
//!
//! The file carries a `format` number so the shape can change later without
//! guessing at what an old file meant. Loading a file from the future fails
//! loudly rather than silently misreading it.
//!
//! Durations serialise as fraction strings — `"1/8"` — so the file stays
//! legible and a hand-edited `"2/4"` loads as the `1/2` it means. See
//! [`crate::duration::as_string`].

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::model::{Document, Track};

/// The format number written by this version.
pub const FORMAT_VERSION: u32 = 1;

/// The on-disk envelope: a version number and the document it describes.
#[derive(Debug, Serialize, Deserialize)]
struct File {
    format: u32,
    document: Document,
}

/// Why a document could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// The text is not the JSON we expect.
    Json(serde_json::Error),
    /// The file was written by a newer version of the format.
    UnsupportedFormat {
        /// The version the file claims.
        found: u32,
        /// The newest version this build understands.
        supported: u32,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Json(error) => write!(f, "not a valid document: {error}"),
            LoadError::UnsupportedFormat { found, supported } => write!(
                f,
                "document is format {found}, but this build understands only up to {supported}"
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Json(error) => Some(error),
            LoadError::UnsupportedFormat { .. } => None,
        }
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(error: serde_json::Error) -> LoadError {
        LoadError::Json(error)
    }
}

/// Serialises a document to compact JSON.
pub fn to_string(document: &Document) -> Result<String, serde_json::Error> {
    serde_json::to_string(&File {
        format: FORMAT_VERSION,
        document: document.clone(),
    })
}

/// Serialises a document to indented JSON, for files humans will open.
pub fn to_string_pretty(document: &Document) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&File {
        format: FORMAT_VERSION,
        document: document.clone(),
    })
}

/// Parses a document, normalising anything a hand edit may have left ragged.
///
/// Durations are reduced as they parse and events are re-sorted by onset, so a
/// loaded document is indistinguishable from one built through the
/// constructors. This does not check the timeline — see [`Document::validate`],
/// which is a separate question a caller may legitimately want to answer "yes,
/// it is ragged, and load it anyway".
pub fn from_str(json: &str) -> Result<Document, LoadError> {
    let file: File = serde_json::from_str(json)?;
    if file.format > FORMAT_VERSION {
        return Err(LoadError::UnsupportedFormat {
            found: file.format,
            supported: FORMAT_VERSION,
        });
    }

    let mut document = file.document;
    document.tracks = document
        .tracks
        .into_iter()
        .map(|track| Track::new(track.name, track.tonic_tpc, track.mode, track.events))
        .collect();
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::{EIGHTH, HALF, QUARTER, ZERO};
    use crate::model::{Mode, Note, Rest, TimeSignature};
    use crate::pitch::{Letter, Tpc};

    fn sample() -> Document {
        Document::new(
            "Sample",
            TimeSignature::COMMON,
            67,
            [Track::new(
                "Melody",
                Tpc::new(Letter::A, -1),
                Mode::Major,
                [
                    Rest::new(ZERO, EIGHTH).into(),
                    Note::new(Tpc::new(Letter::E, -1), 4, EIGHTH, EIGHTH).into(),
                    Note::new(Tpc::new(Letter::A, -1), 4, QUARTER, HALF + QUARTER).into(),
                ],
            )],
        )
    }

    #[test]
    fn round_trips_through_json() {
        let document = sample();
        let json = to_string(&document).unwrap();
        assert_eq!(from_str(&json).unwrap(), document);

        let pretty = to_string_pretty(&document).unwrap();
        assert_eq!(from_str(&pretty).unwrap(), document);
    }

    #[test]
    fn spelling_survives_the_round_trip() {
        // The whole point of the model: A-flat must not come back as G-sharp.
        let document = from_str(&to_string(&sample()).unwrap()).unwrap();
        assert_eq!(document.tracks[0].tonic_tpc.to_string(), "Ab");
        assert_ne!(document.tracks[0].tonic_tpc, Tpc::new(Letter::G, 1));
    }

    #[test]
    fn durations_are_reduced_on_load() {
        // A hand-edited file may write 2/16 where the writer wrote 1/8.
        let json = r#"{
            "format": 1,
            "document": {
                "title": "Hand edited",
                "tracks": [{
                    "name": "Melody",
                    "tonic_tpc": 0,
                    "mode": "Major",
                    "events": [
                        {"kind": "Note", "tpc": 0, "octave": 4,
                         "onset": "0", "dur": "2/16"}
                    ]
                }],
                "time_sig": {"beats": 4, "beat_value": 4},
                "bpm": 67
            }
        }"#;
        let document = from_str(json).unwrap();
        assert_eq!(document.tracks[0].events[0].dur(), EIGHTH);
    }

    #[test]
    fn events_are_sorted_on_load() {
        let mut document = sample();
        document.tracks[0].events.reverse();
        let json = serde_json::to_string(&File {
            format: FORMAT_VERSION,
            document,
        })
        .unwrap();
        let loaded = from_str(&json).unwrap();
        assert_eq!(loaded.tracks[0].events[0].onset(), ZERO);
        assert_eq!(loaded, sample());
    }

    #[test]
    fn a_file_from_the_future_is_refused() {
        let json = to_string(&sample())
            .unwrap()
            .replace("\"format\":1", "\"format\":99");
        match from_str(&json) {
            Err(LoadError::UnsupportedFormat { found, supported }) => {
                assert_eq!((found, supported), (99, FORMAT_VERSION));
            }
            other => panic!("expected a version refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_zero_denominator_is_refused_rather_than_dividing_by_zero() {
        let json = to_string(&sample()).unwrap().replace("\"1/8\"", "\"1/0\"");
        assert!(matches!(from_str(&json), Err(LoadError::Json(_))));
    }

    #[test]
    fn the_written_shape_is_what_we_think_it_is() {
        let json = to_string(&sample()).unwrap();
        // Pitch is a bare integer on the line of fifths, not a note name.
        assert!(json.contains("\"tonic_tpc\":-4"), "{json}");
        // Durations are [numerator, denominator] pairs, never floats.
        assert!(json.contains("\"dur\":\"1/8\""), "{json}");
        assert!(!json.contains('.'), "a float leaked into the file: {json}");
        // Events say what they are.
        assert!(json.contains("\"kind\":\"Rest\""), "{json}");
    }
}
