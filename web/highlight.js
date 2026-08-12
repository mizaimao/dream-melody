// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// Lighting up the noteheads that are sounding.

/**
 * Builds a function that lights the noteheads of a schedule entry, and clears
 * whatever it lit last.
 *
 * Matching is by object identity: the transport and the staff adapter were
 * handed the very same note objects out of one parse, so there is nothing to
 * look up by position and nothing that can fall out of step.
 *
 * Takes the drawn notes from `drawStaff`. A note whose SVG cannot be reached is
 * skipped rather than throwing — a missing highlight is a blemish, and a broken
 * transport is not.
 */
export function createHighlighter(drawn) {
  const elements = new Map();
  for (const { vex, model } of drawn) {
    const element = vex.getSVGElement?.();
    if (element) elements.set(model, element);
  }

  let lit = [];
  const highlight = (entry) => {
    for (const element of lit) element.classList.remove("sounding");
    lit = (entry?.notes ?? []).map((note) => elements.get(note)).filter(Boolean);
    for (const element of lit) element.classList.add("sounding");
  };

  // How many noteheads could actually be reached, so a caller can tell the
  // difference between "nothing is sounding" and "highlighting is broken".
  highlight.reachable = elements.size;
  return highlight;
}
