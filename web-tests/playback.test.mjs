// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// What sounds, when, and at what pitch.
//
// No audio device is involved. The schedule is arithmetic over the render view,
// so it is checked as arithmetic — which catches the things that would actually
// be wrong to the ear: a tie struck twice, a rest that sounds, a tempo applied
// to the wrong unit.
//
// Run with:  node --test 'web-tests/*.test.mjs'

import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { buildSchedule, durationOf, frequencyOf } from "../web/playback.js";

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const { Score } = require("../core/wasm/pkg-node/dream_melody_wasm.js");
const fixture = require("node:fs").readFileSync(
  join(here, "..", "web", "fixture-a.json"),
  "utf8",
);

/** The render view of a score, as the browser gets it. */
function viewOf(json) {
  const score = Score.fromJson(json);
  const view = JSON.parse(score.renderJson());
  score.free();
  return view;
}

/** Within a hundredth of a cent is exact enough to call equal. */
function assertClose(actual, expected, what) {
  assert.ok(
    Math.abs(actual - expected) < 1e-9,
    `${what}: expected ${expected}, got ${actual}`,
  );
}

test("equal temperament is anchored at A440", () => {
  assertClose(frequencyOf(69), 440, "A4");
  assertClose(frequencyOf(81), 880, "A5");
  assertClose(frequencyOf(57), 220, "A3");
  // Middle C, to the usual four figures.
  assert.ok(Math.abs(frequencyOf(60) - 261.6256) < 0.001);
});

test("Fixture A sounds thirteen notes, and the rests stay silent", () => {
  const schedule = buildSchedule(viewOf(fixture));
  assert.equal(schedule.length, 13);

  // Fixture A's pitches: E-flat 4, A-flat 4, B-flat 4, D-flat 5.
  assert.deepEqual(
    schedule.map((entry) => entry.midi),
    [63, 68, 63, 68, 63, 68, 70, 73, 70, 63, 73, 70, 63],
  );
});

test("the first note waits for the rest that opens the bar", () => {
  const view = viewOf(fixture);
  const schedule = buildSchedule(view);

  // At 67 bpm a whole note lasts 240/67 seconds, so an eighth lasts an eighth
  // of that. The opening rest is one eighth, so the first note starts there.
  const whole = 240 / 67;
  assertClose(schedule[0].startSeconds, whole / 8, "first onset");
  assertClose(schedule[0].endSeconds, whole / 4, "first release");
  assertClose(schedule[0].frequency, frequencyOf(63), "first pitch");

  // Two bars of 4/4 at 67 bpm.
  assertClose(durationOf(view), whole * 2, "total length");
});

test("tempo changes what is heard and not what is written", () => {
  const slow = viewOf(fixture);
  const fast = { ...slow, bpm: 134 };
  const at = (view) => buildSchedule(view).map((entry) => entry.startSeconds);

  const slowOnsets = at(slow);
  const fastOnsets = at(fast);
  assert.equal(slowOnsets.length, fastOnsets.length);
  for (let index = 0; index < slowOnsets.length; index += 1) {
    assertClose(fastOnsets[index], slowOnsets[index] / 2, `onset ${index}`);
  }
  // And the pitches are untouched.
  assert.deepEqual(
    buildSchedule(slow).map((entry) => entry.midi),
    buildSchedule(fast).map((entry) => entry.midi),
  );
});

test("a note tied across a barline is struck once, not twice", () => {
  // Three quarters, then a half note starting on beat 4 and running into the
  // next bar, then three quarters.
  const tied = {
    format: 1,
    document: {
      title: "Tied",
      tracks: [
        {
          name: "Melody",
          tonic_tpc: 0,
          mode: "Major",
          register: 4,
          events: [
            { kind: "Note", tpc: 0, octave: 4, onset: "0", dur: "3/4" },
            { kind: "Note", tpc: 0, octave: 4, onset: "3/4", dur: "1/2" },
            { kind: "Note", tpc: 0, octave: 4, onset: "5/4", dur: "3/4" },
          ],
        },
      ],
      time_sig: { beats: 4, beat_value: 4 },
      bpm: 60,
    },
  };
  const view = viewOf(JSON.stringify(tied));

  // The staff draws four noteheads: the tied note is split at the barline.
  const drawn = view.tracks[0].bars.flatMap((bar) => bar.notes);
  assert.equal(drawn.length, 4);
  assert.equal(drawn[1].tied_to_next, true);
  assert.equal(drawn[2].tied_from_previous, true);

  // The ear hears three, and the middle one lasts a full half note.
  const schedule = buildSchedule(view);
  assert.equal(schedule.length, 3, "a tie must not be struck twice");
  assertClose(schedule[1].startSeconds, 3, "tied note onset");
  assertClose(schedule[1].endSeconds, 5, "tied note release");
  // Both noteheads belong to it, so both can be lit while it sounds.
  assert.equal(schedule[1].notes.length, 2);
});

test("a repeated pitch is two notes, not one long one", () => {
  const repeated = {
    format: 1,
    document: {
      title: "Repeated",
      tracks: [
        {
          name: "Melody",
          tonic_tpc: 0,
          mode: "Major",
          register: 4,
          events: [
            { kind: "Note", tpc: 0, octave: 4, onset: "0", dur: "1/2" },
            { kind: "Note", tpc: 0, octave: 4, onset: "1/2", dur: "1/2" },
          ],
        },
      ],
      time_sig: { beats: 4, beat_value: 4 },
      bpm: 60,
    },
  };
  const schedule = buildSchedule(viewOf(JSON.stringify(repeated)));
  assert.equal(schedule.length, 2);
  assert.equal(schedule[0].midi, schedule[1].midi);
  assertClose(schedule[0].endSeconds, schedule[1].startSeconds, "the join");
});

test("every entry sounds for a positive length, in order", () => {
  const schedule = buildSchedule(viewOf(fixture));
  let previous = -1;
  for (const entry of schedule) {
    assert.ok(entry.endSeconds > entry.startSeconds, "a note of no length");
    assert.ok(entry.startSeconds >= previous, "the schedule is out of order");
    assert.ok(entry.frequency > 0, "a note with no pitch");
    previous = entry.startSeconds;
  }
});
