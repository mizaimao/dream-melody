// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// The model -> VexFlow adapter.
//
// VexFlow draws the notes. It owns glyphs, beaming and accidental placement,
// and this file owns none of those: it translates what the model already
// decided into the shapes VexFlow expects, and nothing more.
//
// Everything musical arrives already worked out by the core — where barlines
// fall, which notes are tied across them, what a duration is called. Nothing
// here recomputes any of it.

const VF = globalThis.VexFlow;

/** What notation calls a length, to what VexFlow calls it. */
const DURATION_CODES = {
  whole: "w",
  half: "h",
  quarter: "q",
  eighth: "8",
  "16th": "16",
  "32nd": "32",
  "64th": "64",
  "128th": "128",
};

/** VexFlow draws key signatures from seven flats to seven sharps. */
const KEY_SIGNATURE_LIMIT = 7;

/** The narrowest a bar may be drawn before the staff starts scrolling. */
const MIN_BAR_WIDTH = 200;
/** Room the first bar needs for its clef, key signature and time signature. */
const FIRST_BAR_EXTRA = 100;
const STAVE_LEFT = 10;
const STAVE_TOP = 24;
const STAVE_HEIGHT = 120;
/** Breathing room so the last note of a bar does not sit on the barline. */
const BAR_PADDING = 14;

/** The accidental a spelling carries, as VexFlow spells it. */
function accidentalCode(alter) {
  if (alter === 0) return "n";
  if (alter > 0) return "#".repeat(alter);
  return "b".repeat(-alter);
}

/**
 * A note's key, as VexFlow wants it: "eb/4".
 *
 * The accidental goes in the key so VexFlow knows the pitch. Whether it gets
 * *drawn* is a separate question, answered by applyAccidentals against the key
 * signature.
 */
function keyOf(note) {
  const accidental = note.alter === 0 ? "" : accidentalCode(note.alter);
  return `${note.step.toLowerCase()}${accidental}/${note.octave}`;
}

/**
 * Builds one VexFlow note.
 *
 * A duration notation has no name for — an unusual tuplet — cannot be drawn as
 * a single head. Rather than draw a lie, this returns null and the caller
 * reports it.
 */
function buildNote(note) {
  const code = DURATION_CODES[note.head];
  if (!code) return null;

  const staveNote = new VF.StaveNote({
    // A rest has no pitch, so "b/4" puts it on the middle line where rests
    // belong. Do not reach for alignCenter here: that centres a note
    // *horizontally in the bar*, which silently drags every rest to the middle
    // of its measure and reorders the music.
    keys: note.rest ? ["b/4"] : [keyOf(note)],
    duration: note.rest ? `${code}r` : code,
  });

  for (let dot = 0; dot < note.dots; dot += 1) {
    VF.Dot.buildAndAttach([staveNote], { all: true });
  }
  return staveNote;
}

/**
 * Draws a score onto an element, and returns what was drawn.
 *
 * Returns `{ width, height, notes, problems }`, where `notes` pairs each drawn
 * VexFlow note with the model note it came from — which is what a hit-testing
 * layer will need, and what playback uses to highlight.
 */
export function drawStaff(element, score, options = {}) {
  const trackIndex = options.track ?? 0;
  const track = score.tracks[trackIndex];
  const problems = [];

  element.replaceChildren();

  const bars = track?.bars ?? [];
  const count = Math.max(bars.length, 1);

  // Fill the width available, and only scroll when the bars would be too
  // cramped to read. A score drawn at a fixed width leaves half the page empty
  // on a desktop and overflows on a phone.
  const available = (options.width ?? element.clientWidth ?? 0) - STAVE_LEFT * 2;
  const barWidth = Math.max(
    MIN_BAR_WIDTH,
    (available - FIRST_BAR_EXTRA) / count,
  );
  const width = STAVE_LEFT * 2 + FIRST_BAR_EXTRA + barWidth * count;
  const height = STAVE_TOP + STAVE_HEIGHT;

  const renderer = new VF.Renderer(element, VF.Renderer.Backends.SVG);
  renderer.resize(width, height);
  const context = renderer.getContext();

  // A key signature past seven accidentals cannot be drawn. The model keeps
  // such keys because they are real; the renderer is where they are clamped,
  // and it says so rather than silently drawing the wrong one.
  let keyName = track?.key_name ?? "C";
  if (Math.abs(track?.key_fifths ?? 0) > KEY_SIGNATURE_LIMIT) {
    problems.push(
      `${keyName} major needs ${Math.abs(track.key_fifths)} accidentals; ` +
        `a staff can draw seven, so the key signature is not shown`,
    );
    keyName = null;
  }

  const drawn = [];
  let x = STAVE_LEFT;

  bars.forEach((bar, index) => {
    const first = index === 0;
    const thisWidth = barWidth + (first ? FIRST_BAR_EXTRA : 0);
    const stave = new VF.Stave(x, STAVE_TOP, thisWidth);

    if (first) {
      stave.addClef("treble");
      if (keyName) stave.addKeySignature(keyName);
      stave.addTimeSignature(`${score.beats}/${score.beat_value}`);
    }
    if (index === bars.length - 1) stave.setEndBarType(VF.Barline.type.END);
    stave.setContext(context).draw();

    const staveNotes = [];
    for (const note of bar.notes) {
      const built = buildNote(note);
      if (!built) {
        problems.push(
          `a note of ${note.dur} whole notes has no single notehead; ` +
            `it is not drawn`,
        );
        continue;
      }
      staveNotes.push(built);
      drawn.push({ vex: built, model: note, bar: bar.index, stave });
    }
    if (staveNotes.length === 0) return;

    const voice = new VF.Voice({
      numBeats: score.beats,
      beatValue: score.beat_value,
    });
    // A bar under edit may not be full. Draw what is there rather than throw.
    voice.setStrict(false);
    voice.addTickables(staveNotes);

    // VexFlow decides which accidentals the key signature already implies, so
    // an E-flat in four flats is drawn bare and an E natural gets its sign.
    if (keyName) VF.Accidental.applyAccidentals([voice], keyName);

    const beams = VF.Beam.generateBeams(staveNotes);
    // Ask the stave where the notes may actually go rather than guessing at a
    // margin: it already knows how much room its clef and signatures took.
    const room = stave.getNoteEndX() - stave.getNoteStartX() - BAR_PADDING;
    new VF.Formatter()
      .joinVoices([voice])
      .format([voice], Math.max(room, MIN_BAR_WIDTH / 2));
    voice.draw(context, stave);
    for (const beam of beams) beam.setContext(context).draw();

    x += thisWidth;
  });

  // Ties are drawn last, across the staves they join.
  for (let index = 0; index < drawn.length - 1; index += 1) {
    if (!drawn[index].model.tied_to_next) continue;
    const next = drawn[index + 1];
    if (!next.model.tied_from_previous) continue;
    new VF.StaveTie({
      firstNote: drawn[index].vex,
      lastNote: next.vex,
      firstIndexes: [0],
      lastIndexes: [0],
    })
      .setContext(context)
      .draw();
  }

  return { width, height, notes: drawn, problems };
}
