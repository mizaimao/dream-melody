// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// What actually reaches the page, checked by reading the drawn SVG.
//
// This drives a real headless browser because the bugs worth catching here are
// not in our arithmetic — the core has 100-odd tests for that — but in what
// VexFlow does with what we hand it. The first version of the adapter passed
// `alignCenter` on rests, which quietly dragged every rest to the middle of its
// bar and reordered the music. Nothing but reading the drawn glyphs would have
// caught it.
//
// Run with:  node --test 'web/tests/*.test.mjs'
// Build the wasm first with:  core/wasm/build.sh

import { test, before, after } from "node:test";
import assert from "node:assert/strict";

import { findChrome, serveApp, renderedDom } from "./browser.mjs";

/** The staff position each y coordinate lands on, in treble clef. */
const STAFF_POSITIONS = {
  64: "G5",
  69: "E5",
  74: "D5",
  79: "C5",
  84: "B4",
  89: "A4",
  94: "G4",
  99: "F4",
  104: "E4",
  109: "D4",
};

const NOTEHEAD_BLACK = 0xe0a4;
const REST_EIGHTH = 0xe4e6;

/**
 * Fixture A as it must appear on the staff, in order.
 *
 * These are staff positions, not pitches: the four flats in the key signature
 * are what make the E line an E-flat. That is the point — a correct render
 * draws the notes bare and lets the signature do the work.
 */
const EXPECTED = [
  "REST", "E4", "A4", "E4", "A4", "E4", "A4", "B4",
  "D5", "REST", "B4", "E4", "D5", "B4", "E4",
];

let app;
before(async () => {
  app = await serveApp();
});
after(() => app?.close());

/** The noteheads and rests actually drawn, left to right. */
function drawnSequence(dom) {
  const svg = dom.slice(dom.indexOf("<svg"), dom.indexOf("</svg>"));
  const glyphs = [];
  const pattern =
    /<text[^>]*x="([-\d.]+)"[^>]*y="([-\d.]+)"[^>]*>(.*?)<\/text>/gs;
  for (const match of svg.matchAll(pattern)) {
    const x = Number(match[1]);
    const y = Number(match[2]);
    for (const symbol of match[3]) {
      const code = symbol.codePointAt(0);
      if (code === NOTEHEAD_BLACK) {
        glyphs.push([x, STAFF_POSITIONS[Math.round(y)] ?? `y${y}`]);
      } else if (code === REST_EIGHTH) {
        glyphs.push([x, "REST"]);
      }
    }
  }
  glyphs.sort((left, right) => left[0] - right[0]);
  return glyphs.map(([, what]) => what);
}

test("the page offers a transport once it has a score to play", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }
  const dom = await renderedDom(chrome, app.origin);
  // The button starts disabled in the markup and is enabled only once the
  // score has loaded, so finding it enabled means the whole chain worked.
  assert.match(dom, /<button id="play"[^>]*>Play<\/button>/);
  assert.doesNotMatch(dom, /<button id="play"[^>]*disabled/);
});

test("Fixture A is drawn on the staff, in order, at the right heights", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }

  const dom = await renderedDom(chrome, app.origin);

  assert.match(dom, /<p id="status">Fixture A · Melody · 4\/4 · 67 bpm<\/p>/);
  assert.match(dom, /<p id="key">1 = Ab3 · Major<\/p>/);
  // Nothing went wrong, and the page says so by keeping the list hidden.
  assert.match(dom, /<ul id="problems" hidden=""><\/ul>/);

  assert.deepEqual(drawnSequence(dom), EXPECTED);
});

test("the browser can highlight what it plays", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }

  // A headless browser makes no sound, but it runs the same clock and the same
  // Web Audio scheduling, which is what can actually break.
  const dom = await renderedDom(chrome, app.origin, "/tests/harness.html", [
    "--autoplay-policy=no-user-gesture-required",
  ]);
  const found = dom.match(/<pre id="results">(.*?)<\/pre>/s);
  assert.ok(found, "the harness never reported");
  const report = JSON.parse(found[1]);

  assert.equal(report.error, null, `the harness threw: ${report.error}`);
  assert.deepEqual(report.problems, []);

  // Fixture A: fifteen noteheads and rests drawn, fifteen digits under them,
  // thirteen of the notes sounding.
  assert.equal(report.drawn, 15);
  assert.equal(report.numbers, 15);
  assert.equal(report.scheduled, 13);
  // Every drawn symbol must be reachable, in both notations, or highlighting
  // quietly does nothing.
  assert.equal(report.reachable, 30);

  // Two bars of 4/4 at 67 bpm.
  assert.ok(Math.abs(report.totalSeconds - (240 / 67) * 2) < 0.001);

  // Whatever the follow loop managed to see, it must have been the music: a
  // headless browser advances virtual time while the audio clock stays real,
  // so it may legitimately see nothing at all.
  assert.deepEqual(report.seen, [63, 68, 63].slice(0, report.seen.length));

  // Driving the highlighter is clock-free, so this part is asserted outright.
  // One sounding note lights its notehead and its digit together.
  assert.equal(report.litOnStaff, 1, "the notehead was not lit");
  assert.equal(report.litInNumbers, 1, "the digit was not lit");
  assert.equal(report.litForOneNote, 2, "the two notations disagreed");
  assert.equal(report.litAfterClear, 0, "the highlight was never cleared");
});
