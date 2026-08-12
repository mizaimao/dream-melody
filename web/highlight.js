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
  // A note is usually drawn more than once — as a notehead on the staff and as
  // a digit in the numbers — so each one keeps a list. Keeping a single element
  // per note would leave whichever notation was added first unlit, silently.
  const elements = new Map();
  let reachable = 0;
  for (const { vex, model } of drawn) {
    const element = vex.getSVGElement?.();
    if (!element) continue;
    const already = elements.get(model);
    if (already) already.push(element);
    else elements.set(model, [element]);
    reachable += 1;
  }

  let lit = [];
  const highlight = (entry) => {
    for (const element of lit) element.classList.remove("sounding");
    lit = (entry?.notes ?? []).flatMap((note) => elements.get(note) ?? []);
    for (const element of lit) element.classList.add("sounding");
  };

  // How many drawn symbols could actually be reached, so a caller can tell the
  // difference between "nothing is sounding" and "highlighting is broken".
  highlight.reachable = reachable;
  return highlight;
}
