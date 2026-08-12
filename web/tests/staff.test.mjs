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
import { createServer } from "node:http";
import { execFile } from "node:child_process";
import { readFile, access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const web = dirname(dirname(fileURLToPath(import.meta.url)));

const CHROMES = [
  process.env.CHROME,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Chromium.app/Contents/MacOS/Chromium",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean);

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".wasm": "application/wasm",
};

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

async function findChrome() {
  for (const path of CHROMES) {
    try {
      await access(path);
      return path;
    } catch {
      /* try the next one */
    }
  }
  return null;
}

let server;
let origin;

before(async () => {
  server = createServer(async (request, response) => {
    const asked = new URL(request.url, "http://localhost").pathname;
    const path = join(web, normalize(asked === "/" ? "/index.html" : asked));
    if (!path.startsWith(web)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const body = await readFile(path);
      response.writeHead(200, {
        "content-type": TYPES[extname(path)] ?? "application/octet-stream",
      });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  origin = `http://127.0.0.1:${server.address().port}`;
});

after(() => server?.close());

/** Loads a page in a headless browser and returns its rendered DOM. */
async function renderedDom(chrome, page = "/index.html", extra = []) {
  const { stdout } = await run(
    chrome,
    [
      "--headless",
      "--disable-gpu",
      "--no-sandbox",
      "--virtual-time-budget=8000",
      ...extra,
      "--dump-dom",
      `${origin}${page}`,
    ],
    { maxBuffer: 64 * 1024 * 1024 },
  );
  return stdout;
}

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
  const dom = await renderedDom(chrome);
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

  const dom = await renderedDom(chrome);

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
  const dom = await renderedDom(chrome, "/tests/harness.html", [
    "--autoplay-policy=no-user-gesture-required",
  ]);
  const found = dom.match(/<pre id="results">(.*?)<\/pre>/s);
  assert.ok(found, "the harness never reported");
  const report = JSON.parse(found[1]);

  assert.equal(report.error, null, `the harness threw: ${report.error}`);
  assert.deepEqual(report.problems, []);

  // Fixture A: fifteen noteheads and rests drawn, thirteen of them sounding.
  assert.equal(report.drawn, 15);
  assert.equal(report.scheduled, 13);
  // Every drawn note must be reachable, or highlighting quietly does nothing.
  assert.equal(report.reachable, 15);

  // Two bars of 4/4 at 67 bpm.
  assert.ok(Math.abs(report.totalSeconds - (240 / 67) * 2) < 0.001);

  // Whatever the follow loop managed to see, it must have been the music: a
  // headless browser advances virtual time while the audio clock stays real,
  // so it may legitimately see nothing at all.
  assert.deepEqual(report.seen, [63, 68, 63].slice(0, report.seen.length));

  // Driving the highlighter is clock-free, so this part is asserted outright.
  assert.equal(report.litForOneNote, 1, "a sounding note was not lit");
  assert.equal(report.litAfterClear, 0, "the highlight was never cleared");
});
