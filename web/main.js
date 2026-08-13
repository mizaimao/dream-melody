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
import { drawJianpu } from "./jianpu.js";
import { createPlayer } from "./playback.js";
import { createHighlighter } from "./highlight.js";

const staffElement = document.querySelector("#staff");
const statusElement = document.querySelector("#status");
const keyElement = document.querySelector("#key");
const jianpuElement = document.querySelector("#jianpu");
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
  const numbers = drawJianpu(jianpuElement, view);
  report([...score.problems(), ...drawn.problems]);

  // One highlighter over both drawings, so a sounding note lights up in the
  // staff and in the numbers at the same instant. It is replaced on a redraw,
  // so the transport calls through the variable rather than holding one.
  let highlight = createHighlighter([...drawn.notes, ...numbers.notes]);
  const player = createPlayer(view, {
    onSounding: (entry) => highlight(entry),
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

  // Running inside the desktop shell, say so, so a failure shows up in the
  // terminal rather than only in a developer console nobody opened.
  globalThis.__TAURI__?.core?.invoke("interface_ready", {
    summary: statusElement.textContent,
    problems: [...score.problems(), ...drawn.problems],
  });

  // The same music as text, for anyone who wants to copy it out.
  console.log(score.toJianpu());
  score.free();

  // Redraw on resize so the staff keeps filling the width.
  let pending = 0;
  globalThis.addEventListener("resize", () => {
    clearTimeout(pending);
    pending = setTimeout(() => {
      const again = drawStaff(staffElement, view);
      const numbersAgain = drawJianpu(jianpuElement, view);
      highlight = createHighlighter([...again.notes, ...numbersAgain.notes]);
    }, 120);
  });
}

main().catch((error) => {
  statusElement.textContent = "failed to start";
  report([String(error && error.message ? error.message : error)]);
  console.error(error);
});
