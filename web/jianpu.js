// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// Numbered notation (jianpu/简谱), drawn from scratch.
//
// There is no library for this, which is the point of the project. It is also
// far less work than staff notation: the glyphs are digits, and everything else
// is a straight line or a dot placed relative to one.
//
// What a symbol is made of:
//
//   accidental   ♯ or ♭, small, to the left of the digit
//   digit        1 to 7, or 0 for a rest
//   octave dots  above the digit for each octave up, below for each down
//   beams        lines under the digit, each halving the beat
//   dots         after the digit, each lengthening it by half
//   dashes       after the digit, each holding it another beat
//
// Nothing here decides what those should be. The core worked all of it out —
// degrees, octaves, beams, dots and dashes arrive on every note — so the staff
// and the numbers cannot disagree about the music, only about how it looks.
//
// SVG rather than canvas: the digits are text, and a hit-testing layer wants
// elements rather than a bitmap to search.

const SVG = "http://www.w3.org/2000/svg";

const DIGIT_SIZE = 21;
const SYMBOL_WIDTH = 30;
const DASH_WIDTH = 26;
const BAR_GAP = 22;
const EDGE = 14;
const BASELINE = 46;

/** Space between the digit and the first dot or beam either side of it. */
const OCTAVE_DOT_STEP = 7;
const OCTAVE_DOT_RADIUS = 1.9;
const BEAM_STEP = 4;
const BEAM_HALF_WIDTH = 8;

const HEIGHT = 92;

function element(name, attributes = {}) {
  const made = document.createElementNS(SVG, name);
  for (const [key, value] of Object.entries(attributes)) {
    made.setAttribute(key, String(value));
  }
  return made;
}

/** The accidental sign a degree carries, if any. */
function accidentalSign(alteration) {
  if (alteration === 0) return "";
  return alteration > 0 ? "♯".repeat(alteration) : "♭".repeat(-alteration);
}

/**
 * How wide a symbol is, dashes included.
 *
 * Laid out on a fixed grid rather than proportionally: numbered notation is
 * read as a sequence of beats, and an even grid is how it is printed.
 */
function widthOf(note) {
  return SYMBOL_WIDTH + note.dashes * DASH_WIDTH;
}

/**
 * Draws one symbol at `x`, and returns the group holding it.
 */
function drawSymbol(parent, note, x) {
  const group = element("g", { class: "symbol" });
  const middle = x + SYMBOL_WIDTH / 2;

  if (note.exact) {
    // A length no marks can say. Print the fraction rather than draw something
    // that would be read as a different rhythm.
    group.append(
      element("title", {}),
      Object.assign(
        element("text", {
          x: middle,
          y: BASELINE - DIGIT_SIZE - 4,
          class: "exact",
          "text-anchor": "middle",
        }),
        { textContent: `[${note.exact}]` },
      ),
    );
  }

  const digit = element("text", {
    x: middle,
    y: BASELINE,
    class: "digit",
    "text-anchor": "middle",
  });
  digit.textContent = note.rest ? "0" : String(note.degree);
  group.append(digit);

  if (!note.rest && note.degree_alteration !== 0) {
    const sign = element("text", {
      x: middle - DIGIT_SIZE * 0.42,
      y: BASELINE - DIGIT_SIZE * 0.42,
      class: "accidental",
      "text-anchor": "middle",
    });
    sign.textContent = accidentalSign(note.degree_alteration);
    group.append(sign);
  }

  // Octave dots: above for up, below for down, one per octave.
  const octaves = note.rest ? 0 : note.degree_octave;
  for (let step = 1; step <= Math.abs(octaves); step += 1) {
    const above = octaves > 0;
    group.append(
      element("circle", {
        cx: middle,
        cy: above
          ? BASELINE - DIGIT_SIZE - OCTAVE_DOT_STEP * step + 4
          : BASELINE + OCTAVE_DOT_STEP * step,
        r: OCTAVE_DOT_RADIUS,
        class: "octave",
      }),
    );
  }

  // Beams: lines under the digit, each one halving the beat. They sit below
  // any octave dot, so the two never collide.
  const beamsFrom =
    BASELINE + (octaves < 0 ? OCTAVE_DOT_STEP * Math.abs(octaves) + 4 : 5);
  for (let beam = 0; beam < note.beams; beam += 1) {
    const y = beamsFrom + BEAM_STEP * beam;
    group.append(
      element("line", {
        x1: middle - BEAM_HALF_WIDTH,
        x2: middle + BEAM_HALF_WIDTH,
        y1: y,
        y2: y,
        class: "beam",
      }),
    );
  }

  // Augmentation dots, after the digit.
  for (let dot = 0; dot < note.dots; dot += 1) {
    group.append(
      element("circle", {
        cx: middle + DIGIT_SIZE * 0.5 + dot * 5,
        cy: BASELINE - DIGIT_SIZE * 0.28,
        r: OCTAVE_DOT_RADIUS,
        class: "augmentation",
      }),
    );
  }

  // Dashes, each holding the note another beat.
  for (let dash = 0; dash < note.dashes; dash += 1) {
    const at = x + SYMBOL_WIDTH + dash * DASH_WIDTH;
    group.append(
      element("line", {
        x1: at + 6,
        x2: at + DASH_WIDTH - 6,
        y1: BASELINE - DIGIT_SIZE * 0.3,
        y2: BASELINE - DIGIT_SIZE * 0.3,
        class: "dash",
      }),
    );
  }

  parent.append(group);
  return group;
}

/**
 * Draws a score as numbered notation, and returns what was drawn.
 *
 * Returns `{ width, height, notes }`, where `notes` pairs each drawn group with
 * the model note it came from — the same shape the staff adapter returns, so
 * the two can be highlighted by one mechanism.
 */
export function drawJianpu(container, score, options = {}) {
  const trackIndex = options.track ?? 0;
  const track = score.tracks[trackIndex];
  container.replaceChildren();
  if (!track) return { width: 0, height: HEIGHT, notes: [] };

  const svg = element("svg", { class: "jianpu", height: HEIGHT });
  const drawn = [];
  let x = EDGE;

  track.bars.forEach((bar, index) => {
    if (index > 0) {
      svg.append(
        element("line", {
          x1: x + BAR_GAP / 2,
          x2: x + BAR_GAP / 2,
          y1: BASELINE - DIGIT_SIZE - 2,
          y2: BASELINE + 6,
          class: "barline",
        }),
      );
      x += BAR_GAP;
    }

    for (const note of bar.notes) {
      const group = drawSymbol(svg, note, x);
      // A tie is drawn as an arc joining the two noteheads, which in numbered
      // notation are two digits.
      if (note.tied_from_previous && drawn.length > 0) {
        const from = drawn.at(-1).x + SYMBOL_WIDTH / 2;
        const to = x + SYMBOL_WIDTH / 2;
        const top = BASELINE - DIGIT_SIZE - 10;
        svg.append(
          element("path", {
            d: `M ${from} ${top} Q ${(from + to) / 2} ${top - 8} ${to} ${top}`,
            class: "tie",
          }),
        );
      }
      drawn.push({ group, model: note, bar: bar.index, x });
      x += widthOf(note);
    }
  });

  // The closing double barline.
  x += 4;
  for (const offset of [0, 4]) {
    svg.append(
      element("line", {
        x1: x + offset,
        x2: x + offset,
        y1: BASELINE - DIGIT_SIZE - 2,
        y2: BASELINE + 6,
        class: offset === 0 ? "barline" : "barline thick",
      }),
    );
  }
  x += EDGE;

  svg.setAttribute("width", x);
  svg.setAttribute("viewBox", `0 0 ${x} ${HEIGHT}`);
  container.append(svg);

  // The same shape drawStaff returns, so one highlighter serves both.
  return {
    width: x,
    height: HEIGHT,
    notes: drawn.map(({ group, model, bar }) => ({
      vex: { getSVGElement: () => group },
      model,
      bar,
    })),
  };
}
