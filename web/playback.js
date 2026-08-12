// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// Playback, for verifying pitches and nothing else.
//
// One oscillator. The model is monophonic, so a single oscillator with its
// frequency scheduled ahead of time is not a simplification — it is exactly the
// shape of the music. No soundfonts, no mixer, no effects.
//
// The schedule is built by a pure function so it can be checked without an
// audio device: what sounds, when, and at what frequency is arithmetic, and
// arithmetic should be tested.

/** Concert A, and the MIDI number it sits on. */
const A4_HZ = 440;
const A4_MIDI = 69;

/** Long enough to avoid a click, short enough to still sound like an attack. */
const ATTACK_SECONDS = 0.006;
const RELEASE_SECONDS = 0.02;
/** Silence between two notes, so a repeated pitch is heard twice. */
const GAP_SECONDS = 0.012;
const PEAK_GAIN = 0.22;

/** The frequency of a MIDI note, in equal temperament. */
export function frequencyOf(midi) {
  return A4_HZ * 2 ** ((midi - A4_MIDI) / 12);
}

/** Reads a duration written as `"3/8"`, or `"2"`, as a number. */
function readDuration(text) {
  const [numerator, denominator] = String(text).split("/");
  const divisor = denominator === undefined ? 1 : Number(denominator);
  return Number(numerator) / divisor;
}

/**
 * Works out what sounds and when.
 *
 * Returns entries of `{ startSeconds, endSeconds, midi, frequency, notes }`,
 * where `notes` are the render notes the entry covers — usually one, but two or
 * more where a note is tied across a barline.
 *
 * Tied segments become a single sounding note. They are drawn as two noteheads
 * because notation cannot cross a barline, but they are one attack, and playing
 * them as two would be audibly wrong.
 */
export function buildSchedule(score, trackIndex = 0) {
  const track = score.tracks[trackIndex];
  if (!track) return [];

  // A whole note lasts four quarters, and a quarter lasts 60/bpm seconds.
  const secondsPerWhole = 240 / Math.max(score.bpm, 1);
  const schedule = [];

  for (const bar of track.bars) {
    for (const note of bar.notes) {
      const start = readDuration(note.onset) * secondsPerWhole;
      const end = start + readDuration(note.dur) * secondsPerWhole;

      if (note.rest) continue;

      const previous = schedule.at(-1);
      if (note.tied_from_previous && previous && previous.midi === note.midi) {
        // The same note, still sounding. Extend it rather than striking again.
        previous.endSeconds = end;
        previous.notes.push(note);
        continue;
      }

      schedule.push({
        startSeconds: start,
        endSeconds: end,
        midi: note.midi,
        frequency: frequencyOf(note.midi),
        notes: [note],
      });
    }
  }
  return schedule;
}

/** How long the music runs, in seconds, rests at the end included. */
export function durationOf(score, trackIndex = 0) {
  const track = score.tracks[trackIndex];
  if (!track) return 0;
  const secondsPerWhole = 240 / Math.max(score.bpm, 1);
  let last = 0;
  for (const bar of track.bars) {
    for (const note of bar.notes) {
      last = Math.max(last, readDuration(note.onset) + readDuration(note.dur));
    }
  }
  return last * secondsPerWhole;
}

/*
 * Follow the music with animation frames where they exist, and a timer where
 * they do not. A headless browser may never paint, and the highlight should
 * still keep up.
 */
const scheduleFrame = (callback) =>
  globalThis.requestAnimationFrame?.(callback) ?? setTimeout(callback, 16);
const cancelFrame = (handle) => {
  globalThis.cancelAnimationFrame?.(handle);
  clearTimeout(handle);
};

/**
 * A transport for one score.
 *
 * `onSounding` is called with the schedule entry now sounding, or null in a
 * silence, so a renderer can follow along. `onStop` is called when the music
 * ends or is stopped.
 */
export function createPlayer(score, { trackIndex = 0, onSounding, onStop } = {}) {
  const schedule = buildSchedule(score, trackIndex);
  const total = durationOf(score, trackIndex);

  let context = null;
  let oscillator = null;
  let startedAt = 0;
  let frame = 0;
  let sounding = null;

  function announce(entry) {
    if (entry === sounding) return;
    sounding = entry;
    onSounding?.(entry);
  }

  function follow() {
    if (!context) return;
    const at = context.currentTime - startedAt;
    if (at >= total) {
      stop();
      return;
    }
    announce(
      schedule.find(
        (entry) => at >= entry.startSeconds && at < entry.endSeconds,
      ) ?? null,
    );
    frame = scheduleFrame(follow);
  }

  async function play() {
    stop();
    // An oscillator cannot be restarted once stopped, so each run gets a fresh
    // one. Only ever one sounds at a time.
    context = new (globalThis.AudioContext ?? globalThis.webkitAudioContext)();
    await context.resume();

    const gain = context.createGain();
    gain.gain.value = 0;
    gain.connect(context.destination);

    oscillator = context.createOscillator();
    // A triangle is easier to pitch by ear than a sine and less harsh than a
    // saw. It is a verification tool, not an instrument.
    oscillator.type = "triangle";
    oscillator.connect(gain);

    startedAt = context.currentTime + 0.06;
    for (const entry of schedule) {
      const from = startedAt + entry.startSeconds;
      const until = startedAt + Math.max(entry.endSeconds - GAP_SECONDS, from);
      oscillator.frequency.setValueAtTime(entry.frequency, from);
      gain.gain.setValueAtTime(0, from);
      gain.gain.linearRampToValueAtTime(PEAK_GAIN, from + ATTACK_SECONDS);
      const holdUntil = Math.max(until - RELEASE_SECONDS, from + ATTACK_SECONDS);
      gain.gain.setValueAtTime(PEAK_GAIN, holdUntil);
      gain.gain.linearRampToValueAtTime(0, until);
    }

    oscillator.start(startedAt);
    oscillator.stop(startedAt + total + 0.1);
    frame = scheduleFrame(follow);
  }

  function stop() {
    cancelFrame(frame);
    if (oscillator) {
      try {
        oscillator.stop();
      } catch {
        // Already stopped; nothing to do.
      }
      oscillator.disconnect();
      oscillator = null;
    }
    if (context) {
      context.close();
      context = null;
    }
    if (sounding !== null) announce(null);
    onStop?.();
  }

  return {
    play,
    stop,
    schedule,
    total,
    get playing() {
      return context !== null;
    },
  };
}
