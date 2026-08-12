// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// The JS side of the bindings, exercised the way the browser will use them.
//
// Run with:  node --test 'core/wasm/tests/*.test.mjs'
// Build first with:  core/wasm/build.sh

import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Score, coreVersion } = require("../pkg-node/dream_melody_wasm.js");

/** Fixture A, as the JSON a saved file holds. */
const FIXTURE_A = JSON.stringify({
  format: 1,
  document: {
    title: "Fixture A",
    tracks: [
      {
        name: "Melody",
        tonic_tpc: -4,
        mode: "Major",
        register: 3,
        events: [
          { kind: "Rest", onset: "0", dur: "1/8" },
          { kind: "Note", tpc: -3, octave: 4, onset: "1/8", dur: "1/8" },
          { kind: "Note", tpc: -4, octave: 4, onset: "1/4", dur: "1/8" },
          { kind: "Note", tpc: -3, octave: 4, onset: "3/8", dur: "1/8" },
          { kind: "Note", tpc: -4, octave: 4, onset: "1/2", dur: "1/8" },
          { kind: "Note", tpc: -3, octave: 4, onset: "5/8", dur: "1/8" },
          { kind: "Note", tpc: -4, octave: 4, onset: "3/4", dur: "1/8" },
          { kind: "Note", tpc: -2, octave: 4, onset: "7/8", dur: "1/8" },
        ],
      },
    ],
    time_sig: { beats: 4, beat_value: 4 },
    bpm: 67,
  },
});

test("the module loads and reports a version", () => {
  assert.match(coreVersion(), /^\d+\.\d+\.\d+$/);
});

test("a score reads and describes itself", () => {
  const score = Score.fromJson(FIXTURE_A);
  assert.equal(score.title, "Fixture A");
  assert.equal(score.bpm, 67);
  assert.equal(score.timeSignature, "4/4");
  assert.equal(score.trackCount, 1);
  assert.equal(score.trackName(0), "Melody");
  assert.equal(score.trackKey(0), "Ab");
  assert.deepEqual(score.problems(), []);
  score.free();
});

test("every format comes out of the browser build", () => {
  const score = Score.fromJson(FIXTURE_A);

  const jianpu = score.toJianpu();
  assert.match(jianpu, /1 = Ab3/);
  assert.match(jianpu, /\| 0_ 5_ 1\^_ 5_ 1\^_ 5_ 1\^_ 2\^_ \|/);

  assert.match(score.toMusicXml(), /<score-partwise version="4\.0">/);
  assert.match(score.toMusicXml(), /<step>E<\/step><alter>-1<\/alter>/);
  assert.match(score.toAbc(), /K:Ab/);

  const midi = score.toMidi();
  assert.ok(midi instanceof Uint8Array);
  assert.deepEqual([...midi.slice(0, 4)], [...Buffer.from("MThd")]);

  score.free();
});

test("every format goes back in", () => {
  const score = Score.fromJson(FIXTURE_A);
  const json = score.toJson();

  const viaJianpu = Score.fromJianpu(score.toJianpu());
  assert.equal(viaJianpu.toJson(), json, "jianpu did not round trip");

  const viaXml = Score.fromMusicXml(score.toMusicXml());
  assert.equal(viaXml.toJson(), json, "MusicXML did not round trip");

  // MIDI keeps the timing but cannot keep spelling, so only timing is claimed.
  const viaMidi = Score.fromMidi(score.toMidi());
  assert.equal(viaMidi.bpm, 67);
  assert.equal(viaMidi.timeSignature, "4/4");

  for (const each of [score, viaJianpu, viaXml, viaMidi]) each.free();
});

test("transposing moves the key and leaves the digits alone", () => {
  const score = Score.fromJson(FIXTURE_A);
  const digits = (s) =>
    s
      .toJianpu()
      .split("\n")
      .find((line) => line.startsWith("|"));

  const up = score.transposeBySemitones(1);
  assert.equal(up.trackKey(0), "A");
  assert.equal(digits(up), digits(score), "the digits moved");

  const sharpward = score.transposeByFifths(9);
  assert.equal(sharpward.trackKey(0), "B");
  assert.equal(digits(sharpward), digits(score), "the digits moved");

  for (const each of [score, up, sharpward]) each.free();
});

test("a ragged score loads and reports what is wrong with it", () => {
  const ragged = JSON.parse(FIXTURE_A);
  ragged.document.tracks[0].events.pop();
  const score = Score.fromJson(JSON.stringify(ragged));

  const problems = score.problems();
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^track 1: /);
  score.free();
});

test("rubbish throws an Error a caller can catch and read", () => {
  assert.throws(() => Score.fromJson("{"), /Error/);
  assert.throws(() => Score.fromJianpu("| 9 |"), /degree/);
  assert.throws(() => Score.fromMusicXml("<html/>"), /Error/);
  assert.throws(() => Score.fromMidi(new Uint8Array([1, 2, 3])), /Error/);
});
