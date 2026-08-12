// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// Stage 7: Fixture A, on a staff, and audible.
//
// The whole musical contribution of this file is zero. It loads the score,
// asks the core what to draw, and hands that to the adapter and the transport.

import init, { Score } from "./wasm/dream_melody_wasm.js";
import { drawStaff } from "./staff.js";
import { createPlayer } from "./playback.js";
import { createHighlighter } from "./highlight.js";

const staffElement = document.querySelector("#staff");
const statusElement = document.querySelector("#status");
const keyElement = document.querySelector("#key");
const problemsElement = document.querySelector("#problems");
const playElement = document.querySelector("#play");

function report(problems) {
  problemsElement.replaceChildren();
  problemsElement.hidden = problems.length === 0;
  for (const problem of problems) {
    const item = document.createElement("li");
    item.textContent = problem;
    problemsElement.append(item);
  }
}

async function main() {
  await init({ module_or_path: "./wasm/dream_melody_wasm_bg.wasm" });

  const response = await fetch("./fixture-a.json");
  if (!response.ok) throw new Error(`cannot load the score: ${response.status}`);
  const score = Score.fromJson(await response.text());

  const view = JSON.parse(score.renderJson());
  const track = view.tracks[0];

  document.title = `${view.title} — Dream Melody`;
  statusElement.textContent =
    `${view.title} · ${track.name} · ${view.beats}/${view.beat_value} · ` +
    `${view.bpm} bpm`;
  keyElement.textContent = `1 = ${track.tonic}${track.register} · ${track.mode}`;

  const drawn = drawStaff(staffElement, view);
  report([...score.problems(), ...drawn.problems]);

  const highlight = createHighlighter(drawn.notes);
  const player = createPlayer(view, {
    onSounding: highlight,
    onStop: () => {
      playElement.textContent = "Play";
      playElement.dataset.playing = "false";
      highlight(null);
    },
  });

  playElement.disabled = false;
  playElement.addEventListener("click", async () => {
    if (player.playing) {
      player.stop();
      return;
    }
    playElement.textContent = "Stop";
    playElement.dataset.playing = "true";
    // A browser will not make a sound until a gesture asks it to, which is what
    // this click is.
    await player.play();
  });

  // Handy while this is read-only: the same score, in the other notation.
  console.log(score.toJianpu());
  score.free();
}

main().catch((error) => {
  statusElement.textContent = "failed to start";
  report([String(error && error.message ? error.message : error)]);
  console.error(error);
});
