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

//! Browser bindings. The same core, running in the browser.
//!
//! No music theory lives here. Every method forwards to `dream-melody-core`
//! and translates its errors into ones JavaScript can catch, so a fix in the
//! model is a fix in the browser without anything being reimplemented.
//!
//! Documents cross the boundary as JSON — the same JSON a saved file holds —
//! rather than as a bespoke object graph, so there is one shape to learn and
//! one code path to keep correct.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use dream_melody_core::interval::Interval;
use dream_melody_core::model::Document;
use dream_melody_core::pitch::Tpc;
use dream_melody_core::{abc, jianpu, json, midi, musicxml};
use wasm_bindgen::prelude::*;

/// Turns any error into a message, since that is all JavaScript will see.
fn why(error: impl core::fmt::Display) -> String {
    error.to_string()
}

/// Turns a message into an error JavaScript can catch.
///
/// `JsError` exists only on wasm, so every reader below returns a plain
/// `Result<_, String>` and this is the single place that converts. That keeps
/// the failure paths testable on a native build, where a `JsError` cannot even
/// be constructed.
fn fail(message: String) -> JsError {
    JsError::new(&message)
}

/// Reads a score from the JSON a saved file holds.
fn read_json(text: &str) -> Result<Document, String> {
    json::from_str(text).map_err(why)
}

/// Reads a score from numbered notation (jianpu/简谱).
fn read_jianpu(text: &str) -> Result<Document, String> {
    jianpu::from_str(text).map_err(why)
}

/// Reads a score from MusicXML.
fn read_music_xml(text: &str) -> Result<Document, String> {
    musicxml::from_str(text).map_err(why)
}

/// Reads a score from a standard MIDI file.
fn read_midi(bytes: &[u8]) -> Result<Document, String> {
    midi::from_bytes(bytes).map_err(why)
}

/// A score, held on the Rust side.
///
/// JavaScript keeps a handle and calls methods on it. Freeing is explicit —
/// call `free()` when done, as with every wasm-bindgen object.
#[wasm_bindgen]
pub struct Score {
    inner: Document,
}

#[wasm_bindgen]
impl Score {
    /// Reads a score from the JSON a saved file holds.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str) -> Result<Score, JsError> {
        read_json(text).map(|inner| Score { inner }).map_err(fail)
    }

    /// Reads a score from numbered notation (jianpu/简谱).
    #[wasm_bindgen(js_name = fromJianpu)]
    pub fn from_jianpu(text: &str) -> Result<Score, JsError> {
        read_jianpu(text).map(|inner| Score { inner }).map_err(fail)
    }

    /// Reads a score from MusicXML, which is what other notation programs
    /// export.
    #[wasm_bindgen(js_name = fromMusicXml)]
    pub fn from_music_xml(text: &str) -> Result<Score, JsError> {
        read_music_xml(text)
            .map(|inner| Score { inner })
            .map_err(fail)
    }

    /// Reads a score from a standard MIDI file.
    ///
    /// MIDI has no spelling, so notes are spelled to suit the key signature the
    /// file declares. Use MusicXML when spelling must survive.
    #[wasm_bindgen(js_name = fromMidi)]
    pub fn from_midi(bytes: &[u8]) -> Result<Score, JsError> {
        read_midi(bytes).map(|inner| Score { inner }).map_err(fail)
    }

    /// Writes the score as JSON, which is what a saved file holds.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsError> {
        json::to_string_pretty(&self.inner).map_err(|error| fail(why(error)))
    }

    /// Writes the score as numbered notation (jianpu/简谱).
    #[wasm_bindgen(js_name = toJianpu)]
    pub fn to_jianpu(&self) -> Result<String, JsError> {
        jianpu::to_string(&self.inner).map_err(|error| fail(why(error)))
    }

    /// Writes the score as MusicXML.
    #[wasm_bindgen(js_name = toMusicXml)]
    pub fn to_music_xml(&self) -> Result<String, JsError> {
        musicxml::to_string(&self.inner).map_err(|error| fail(why(error)))
    }

    /// Writes the score as ABC.
    #[wasm_bindgen(js_name = toAbc)]
    pub fn to_abc(&self) -> Result<String, JsError> {
        abc::to_string(&self.inner).map_err(|error| fail(why(error)))
    }

    /// Writes the score as a standard MIDI file.
    #[wasm_bindgen(js_name = toMidi)]
    pub fn to_midi(&self) -> Result<Vec<u8>, JsError> {
        midi::to_bytes(&self.inner).map_err(|error| fail(why(error)))
    }

    /// Moves the score along the line of fifths, keeping every scale degree.
    ///
    /// The octave is chosen so the music moves as little as possible, which is
    /// what "transpose to this key" ordinarily means.
    #[wasm_bindgen(js_name = transposeByFifths)]
    pub fn transpose_by_fifths(&self, fifths: i32) -> Score {
        Score {
            inner: self.inner.transpose(Interval::simplest(fifths)),
        }
    }

    /// Moves the score by a number of semitones, spelling it for the key it is
    /// leaving: up one semitone from C major gives D-flat, not C-sharp.
    #[wasm_bindgen(js_name = transposeBySemitones)]
    pub fn transpose_by_semitones(&self, semitones: i32) -> Score {
        let tonic = self
            .inner
            .tracks
            .first()
            .map_or(Tpc::C, |track| track.tonic_tpc);
        Score {
            inner: self
                .inner
                .transpose(Interval::by_semitones(semitones, tonic)),
        }
    }

    /// Every way the score's timeline is malformed, as readable sentences.
    ///
    /// Empty means it is well formed. This is a question, not a refusal: a
    /// score under edit is legitimately ragged between keystrokes.
    pub fn problems(&self) -> Vec<String> {
        self.inner
            .validate()
            .into_iter()
            .map(|(track, issue)| format!("track {}: {issue}", track + 1))
            .collect()
    }

    /// The score's title.
    #[wasm_bindgen(getter)]
    pub fn title(&self) -> String {
        self.inner.title.clone()
    }

    /// Tempo, in quarter notes per minute.
    #[wasm_bindgen(getter)]
    pub fn bpm(&self) -> u32 {
        self.inner.bpm
    }

    /// The time signature, as written: `"4/4"`.
    #[wasm_bindgen(getter, js_name = timeSignature)]
    pub fn time_signature(&self) -> String {
        self.inner.time_sig.to_string()
    }

    /// How many voices the score holds.
    #[wasm_bindgen(getter, js_name = trackCount)]
    pub fn track_count(&self) -> usize {
        self.inner.tracks.len()
    }

    /// The name of a voice, or an empty string if there is no such voice.
    #[wasm_bindgen(js_name = trackName)]
    pub fn track_name(&self, track: usize) -> String {
        self.inner
            .tracks
            .get(track)
            .map(|track| track.name.clone())
            .unwrap_or_default()
    }

    /// The key a voice is written in, as a spelling such as `"Ab"`.
    #[wasm_bindgen(js_name = trackKey)]
    pub fn track_key(&self, track: usize) -> String {
        self.inner
            .tracks
            .get(track)
            .map(|track| track.tonic_tpc.to_string())
            .unwrap_or_default()
    }
}

/// The version of the core this build wraps.
#[wasm_bindgen(js_name = coreVersion)]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two bars of Fixture A, as the JSON a saved file holds.
    const FIXTURE: &str = r#"{
        "format": 1,
        "document": {
            "title": "Fixture A",
            "tracks": [{
                "name": "Melody",
                "tonic_tpc": -4,
                "mode": "Major",
                "register": 3,
                "events": [
                    {"kind": "Rest", "onset": "0", "dur": "1/8"},
                    {"kind": "Note", "tpc": -3, "octave": 4, "onset": "1/8", "dur": "1/8"},
                    {"kind": "Note", "tpc": -4, "octave": 4, "onset": "1/4", "dur": "1/8"},
                    {"kind": "Note", "tpc": -3, "octave": 4, "onset": "3/8", "dur": "1/8"},
                    {"kind": "Note", "tpc": -4, "octave": 4, "onset": "1/2", "dur": "1/2"}
                ]
            }],
            "time_sig": {"beats": 4, "beat_value": 4},
            "bpm": 67
        }
    }"#;

    fn score_of(text: &str) -> Score {
        Score {
            inner: read_json(text).unwrap(),
        }
    }

    #[test]
    fn a_score_reads_and_reports_itself() {
        let score = score_of(FIXTURE);
        assert_eq!(score.title(), "Fixture A");
        assert_eq!(score.bpm(), 67);
        assert_eq!(score.time_signature(), "4/4");
        assert_eq!(score.track_count(), 1);
        assert_eq!(score.track_name(0), "Melody");
        assert_eq!(score.track_key(0), "Ab");
        assert!(score.problems().is_empty());
    }

    #[test]
    fn every_format_comes_out() {
        let score = score_of(FIXTURE);
        assert!(score.to_json().unwrap().contains("Fixture A"));
        assert!(score.to_jianpu().unwrap().contains("1 = Ab3"));
        assert!(score.to_music_xml().unwrap().contains("<score-partwise"));
        assert!(score.to_abc().unwrap().contains("K:Ab"));
        assert_eq!(&score.to_midi().unwrap()[0..4], b"MThd");
    }

    #[test]
    fn every_format_goes_back_in() {
        let score = score_of(FIXTURE);

        let from_jianpu = Score {
            inner: read_jianpu(&score.to_jianpu().unwrap()).unwrap(),
        };
        assert_eq!(from_jianpu.to_json().unwrap(), score.to_json().unwrap());

        let from_xml = Score {
            inner: read_music_xml(&score.to_music_xml().unwrap()).unwrap(),
        };
        assert_eq!(from_xml.to_json().unwrap(), score.to_json().unwrap());

        // MIDI keeps the timing but not the spelling, so only timing is claimed.
        let from_midi = Score {
            inner: read_midi(&score.to_midi().unwrap()).unwrap(),
        };
        assert_eq!(from_midi.bpm(), 67);
        assert_eq!(from_midi.time_signature(), "4/4");
    }

    #[test]
    fn transposition_keeps_the_digits_and_moves_the_key() {
        let score = score_of(FIXTURE);
        let body = |score: &Score| -> String {
            score
                .to_jianpu()
                .unwrap()
                .lines()
                .find(|line| line.starts_with('|'))
                .unwrap_or_default()
                .to_string()
        };

        let up = score.transpose_by_semitones(1);
        // A-flat up a semitone is A, which has fewer accidentals than B-double-flat.
        assert_eq!(up.track_key(0), "A");
        assert_eq!(body(&up), body(&score));

        let sharpward = score.transpose_by_fifths(9);
        assert_eq!(sharpward.track_key(0), "B");
        assert_eq!(body(&sharpward), body(&score));
    }

    #[test]
    fn a_ragged_score_reports_its_problems_rather_than_refusing_to_load() {
        let ragged = FIXTURE.replace(
            r#""onset": "1/2", "dur": "1/2""#,
            r#""onset": "1/2", "dur": "1/4""#,
        );
        let score = score_of(&ragged);
        let problems = score.problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("track 1: "), "{problems:?}");
    }

    /// Rubbish must come back as a message, not a panic. Tested against the
    /// plain readers because a `JsError` cannot exist on a native build.
    #[test]
    fn rubbish_is_refused_rather_than_panicking() {
        assert!(read_json("{").is_err());
        assert!(read_jianpu("| 9 |").is_err());
        assert!(read_music_xml("<html/>").is_err());
        assert!(read_midi(b"nope").is_err());
        // And the message says something useful rather than "error".
        let complaint = read_jianpu("| 9 |").unwrap_err();
        assert!(complaint.contains("degree"), "{complaint}");
    }
}
