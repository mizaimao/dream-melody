// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// The numbered notation, read back off the page.
//
// There is no library drawing this, so there is nothing to trust: every dot and
// every beam is ours. The test reads the drawn SVG and reconstructs the symbols
// as text, then compares that with the text format the core writes for the same
// score. If the drawing and the writing ever disagree, one of them is wrong.
//
// Run with:  node --test 'web-tests/*.test.mjs'

import { test, before, after } from "node:test";
import assert from "node:assert/strict";

import { findChrome, serveApp, renderedDom, insideOf } from "./browser.mjs";

/** Where the digit's baseline sits, from web/jianpu.js. */
const BASELINE = 46;

let app;
before(async () => {
  app = await serveApp();
});
after(() => app?.close());

/**
 * Reads the drawn symbols back as text, in the same shape the core's jianpu
 * writer uses: `1^_` is degree one, an octave up, with one beam.
 */
function symbolsFrom(dom) {
  const block = insideOf(dom, "jianpu");
  const groups = block.match(/<g class="symbol">.*?<\/g>/gs) ?? [];

  return groups.map((group) => {
    const digit = group.match(/class="digit"[^>]*>([^<]*)</)?.[1] ?? "?";
    const accidental = group.match(/class="accidental"[^>]*>([^<]*)</)?.[1] ?? "";

    let up = 0;
    let down = 0;
    for (const circle of group.match(/<circle[^>]*><\/circle>/g) ?? []) {
      if (!/class="octave"/.test(circle)) continue;
      const cy = Number(circle.match(/cy="([-\d.]+)"/)?.[1]);
      if (cy < BASELINE) up += 1;
      else down += 1;
    }

    const beams = (group.match(/class="beam"/g) ?? []).length;
    const dots = (group.match(/class="augmentation"/g) ?? []).length;
    const dashes = (group.match(/class="dash"/g) ?? []).length;

    const sign = accidental.replaceAll("♯", "#").replaceAll("♭", "b");
    return (
      sign +
      digit +
      "^".repeat(up) +
      "v".repeat(down) +
      "_".repeat(beams) +
      ".".repeat(dots) +
      " -".repeat(dashes)
    );
  });
}

test("the numbers drawn are the numbers the SPEC gives for Fixture A", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }

  const dom = await renderedDom(chrome, app.origin);
  const drawn = symbolsFrom(dom);

  // SPEC section 5, with the durations made explicit:
  //   | 0_ 5_ 1^_ 5_ 1^_ 5_ 1^_ 2^_ | 4^_ 0_ 2^_ 5_ 4^ 2^_ 5_ |
  assert.deepEqual(drawn, [
    "0_", "5_", "1^_", "5_", "1^_", "5_", "1^_", "2^_",
    "4^_", "0_", "2^_", "5_", "4^", "2^_", "5_",
  ]);
});

test("the drawing and the text format agree, symbol for symbol", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }

  const dom = await renderedDom(chrome, app.origin);
  const drawn = symbolsFrom(dom).join(" ");

  // The page logs what the core writes. Compare the two directly: the drawing
  // is ours and the text is the core's, and they must say the same thing.
  const { createRequire } = await import("node:module");
  const require = createRequire(import.meta.url);
  const { Score } = require("../core/wasm/pkg-node/dream_melody_wasm.js");
  const fixture = require("node:fs").readFileSync(
    new URL("../web/fixture-a.json", import.meta.url),
    "utf8",
  );
  const score = Score.fromJson(fixture);
  const written = score
    .toJianpu()
    .split("\n")
    .find((line) => line.startsWith("|"));
  score.free();

  // Strip the barlines the text uses; the drawing marks them differently.
  const asText = written
    .replaceAll("|", " ")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
  assert.equal(drawn, asText);
});

test("the octave dot sits above the digit for a higher octave", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }

  const dom = await renderedDom(chrome, app.origin);
  const block = insideOf(dom, "jianpu");
  const groups = block.match(/<g class="symbol">.*?<\/g>/gs) ?? [];

  // The third symbol is degree 1 an octave above the register, so its dot must
  // be above the digit rather than below it.
  const dot = groups[2].match(/<circle[^>]*class="octave"[^>]*><\/circle>/)?.[0];
  assert.ok(dot, "degree 1 an octave up has no octave dot");
  assert.ok(Number(dot.match(/cy="([-\d.]+)"/)[1]) < BASELINE);

  // The rest, and degree 5 in the register itself, carry no dot at all.
  assert.doesNotMatch(groups[0], /class="octave"/);
  assert.doesNotMatch(groups[1], /class="octave"/);
});

test("a bar line separates the bars, and the score ends with a double", async (t) => {
  const chrome = await findChrome();
  if (!chrome) {
    t.skip("no headless Chrome found; set CHROME to run this");
    return;
  }

  const dom = await renderedDom(chrome, app.origin);
  const block = insideOf(dom, "jianpu");
  const bars = (block.match(/class="barline[^"]*"/g) ?? []).length;
  // One between the two bars, then two for the closing double barline.
  assert.equal(bars, 3);
  assert.equal((block.match(/class="barline thick"/g) ?? []).length, 1);
});
