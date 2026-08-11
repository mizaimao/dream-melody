# Dream Melody — SPEC.md

A notation editor that stores **scale degrees, not pitches** — so a score can be
rendered as standard staff notation, as numbered notation (jianpu/简谱), or as
model-ready text, from one source of truth.

**Terminology.** In prose, docs, and user-facing strings, always write
`numbered notation (jianpu/简谱)` on first mention so readers of any background
recognise it. In code, use `jianpu` alone — ASCII, single word, valid
identifier.

Written for an implementing agent. Read this whole file before writing code.

---

## 1. Why this exists

Existing notation tools store absolute pitch and treat numbered notation as an
export filter bolted on top. That gets enharmonics wrong, makes transposition a
destructive transformation, and means the very large population who read
numbered notation (jianpu/简谱) rather than staff notation are second-class
users of every tool that exists.

Here, spelling is stored, key is a document property, and every rendering —
staff, jianpu, ABC, MIDI — is a pure function of the same model.

## 2. Non-goals

Do not build these. They are explicitly out of scope and any PR adding them
should be rejected.

- Professional engraving. No optical spacing, no collision avoidance, no
  cross-staff beaming. Notes on lines, correct rhythm, readable. That is all.
- Polyphony within a voice. One note sounding at a time, per track.
- A custom file format. Persistence is JSON; interchange is MusicXML and MIDI.
- Playback beyond pitch verification. One oscillator. No soundfonts, no mixer,
  no VST hosting, no effects.
- Lyrics, dynamics, articulations, ornaments, repeats, codas.
- Plugins, palettes, an inspector panel, or anything resembling MuseScore's
  chrome.

## 3. Data model

### Pitch: tonal pitch class

Pitch is stored as a signed integer position on the **line of fifths**, with
C = 0. This is what distinguishes G# from Ab — they sound identical but are
different values here, which is the entire reason the model works.

```
... Fb  Cb  Gb  Db  Ab  Eb  Bb  F   C   G   D   A   E   B   F#  C#  G# ...
    -8  -7  -6  -5  -4  -3  -2  -1  0   1   2   3   4   5   6   7   8
```

Signed and unbounded. Do **not** use an unsigned range with a floor at Fbb —
that bakes in a double-accidental ceiling that breaks when transposing C-flat
major upward. Clamp in the renderer, where a triple flat genuinely cannot be
drawn, not in the model.

Derivations:

```
letter     = mod(4 * tpc, 7)                    // 0..6 -> C D E F G A B
accidental = floor((tpc + 1) / 7)               // -1 flat, 0 natural, +1 sharp
midi       = (octave + 1) * 12
             + [0,2,4,5,7,9,11][letter]
             + accidental
transpose  = tpc + k                            // the entire operation
```

**Octave numbering is letter-based, not pitch-based.** B#4 is octave 4 and
sounds as MIDI 72, not 60. Write a test for this specifically; it is the
classic bug.

Scientific pitch notation: C4 = middle C = MIDI 60.

### Duration: exact rationals

Never floats. Triplets and dotted-triplet-ties must stay exact under addition.
Use a rational type (`num-rational` in Rust). Unit is the whole note, so a
quarter is `1/4`.

### Structures

```rust
struct Note {
    tpc:    i32,       // line-of-fifths position, C = 0
    octave: i32,       // scientific, C4 = middle C
    onset:  Rational,  // whole notes from start of track
    dur:    Rational,  // whole notes
}

struct Rest {
    onset: Rational,
    dur:   Rational,
}

struct Track {
    name:      String,
    tonic_tpc: i32,     // per-track: transposing instruments, capo
    mode:      Mode,    // Major | Minor | Dorian | Mixolydian | ...
    events:    Vec<Event>,   // Note | Rest, sorted by onset
}

struct Document {
    tracks:    Vec<Track>,
    time_sig:  (u32, u32),
    bpm:       u32,
    title:     String,
}
```

**Ties merge. A tie is not stored.**

In standard notation, a tie joins two noteheads of the *same pitch* and they are
performed as a single sustained note with the durations added — a quarter tied
to a quarter sounds exactly as a half note, one attack. Ties exist only because
notation cannot draw a note crossing a barline, and cannot express certain
durations with a single symbol.

A tie is therefore an artifact of the notation, not a musical event. The model
stores the music; the renderer deals with the barline constraint. Two tied
noteheads are one `Note` with the summed rational duration, and the renderer
splits it back out when drawing. "How long is this note" always has a direct
answer.

**Slurs are not ties and are out of scope.** A slur is drawn with the same
curved line but connects notes of *different* pitches and means play smoothly —
each note keeps its own attack. Disambiguation on import is pitch equality: same
pitch means tie, different pitch means slur. Slurs are discarded on import; do
not model them.

**Multi-track from day one in the model, single-track in the UI for v1.**
Looping over a one-element vector is free; retrofitting `tracks` onto a flat
note list is not.

Per-track `tonic_tpc` is load-bearing: a B-flat clarinet part and a capo-1
guitar part are the same mechanism.

### Deriving a scale degree

For jianpu, degree comes from `note.tpc - tonic_tpc` mapped through the line of
fifths. This yields both the digit (1–7) and its accidental deterministically.
No heuristics, no lookup tables keyed on key name.

## 4. Invariants

These are the acceptance tests. Write them first.

1. **Round trip is lossless.**
   `staff -> model -> jianpu -> model -> staff` must be identity, for every key
   signature including those requiring double accidentals.

2. **Transposition preserves spelling correctness.**
   Transposing up a minor second from C major yields D-flat major (flats), not
   C-sharp major (sharps), when the target key signature has fewer accidentals.

3. **Every bar sums exactly to the time signature.**
   Bar validation is a model-level check, not a UI convenience. A document that
   violates it is invalid and cannot be saved.

4. **MIDI export round-trips onset and duration exactly.**
   Rational arithmetic in, no float drift out.

## 5. Test fixtures

Ship these as fixtures before implementing anything.

**Fixture A — the reference melody.** Tempo 67, 4/4, tonic A-flat.

```
| 0  5  1^ 5  1^ 5  1^ 2^ | 4^ 0  2^ 5  4^~4^ 2^ 5 |
```

Exercises: rest handling, off-beat entry, octave displacement, a tie, and the
G#/Ab spelling ambiguity. In ABC:

```
X:1
M:4/4
L:1/8
Q:1/4=67
K:Ab
z E A E A E A B | d z B E d2 B E |]
```

**Fixture B — enharmonic transposition.** Fixture A transposed to B major.
Must produce sharps.

**Fixture C — double accidentals.** Fixture A transposed to C-flat major.
Must produce a double flat and must not panic.

**Fixture D — the octave bug.** A single B#4. Must report MIDI 72.

## 6. Architecture

```
dream-melody/
├── Cargo.toml       workspace root
├── core/            dream-melody-core — model, TPC math, transposition,
│                    serializers. Zero UI dependencies. Exhaustively tested.
├── core/wasm/       dream-melody-wasm — wasm-bindgen wrapper. Same core,
│                    running in the browser.
├── web/             Static ES modules, no bundler. Canvas editor + VexFlow.
├── src-tauri/       Tauri desktop shell. Thin; delegates to core.
└── SPEC.md
```

Package and repo names are kebab-case (`dream-melody`); the Rust import path is
`dream_melody`, which Cargo derives automatically.

One core, three consumers. A fix in the model fixes every surface.

Rendering standard notation uses **VexFlow** (MIT). Do not write a glyph
engine, a beaming algorithm, or accidental placement logic. Write the
model -> VexFlow adapter and a hit-testing layer for mouse interaction.

The jianpu renderer **is** written from scratch — that is the novel part.

Playback is Web Audio `OscillatorNode`. Nothing more.

## 7. Build order

Strictly sequential. Do not start a stage before the previous one's tests pass.

1. `core`: `Note`, `Rational` durations, TPC conversions. Fixture D passes.
2. `core`: transposition. Fixtures B and C pass.
3. `core`: jianpu serializer. Invariant 1 passes.
4. `core`: ABC and MIDI export. Invariant 4 passes.
5. `core/wasm`: bindings, smoke-tested from JS.
6. `web`: VexFlow render of a hardcoded Fixture A. Read-only.
7. `web`: Web Audio playback of the rendered document.
8. `web`: jianpu rendered below the staff, live.
9. `web`: click-to-place note entry, duration selector, bar validation.
10. `src-tauri`: desktop shell.

**The UI is last on purpose.** It is the fastest part to write and the part
most likely to be thrown away. Everything above it must be correct first.

Milestone for a public demo: **stage 9.** Draw Fixture A by hand, hear it,
watch the numbered notation appear. That is the whole pitch.

## 8. License

**AGPL-3.0** for the entire repository. Single license, no split.

Anyone who uses this code must publish their own code — including if they only
run it as a network service. That network clause is the point: plain GPL lets a
hosted service use the code without releasing anything, because hosting is not
distribution. AGPL closes that.

`LICENSE` contains the full AGPL-3.0 text. Every source file gets the standard
header. `README.md` states it plainly.

`CONTRIBUTING.md` must exist **before the first pull request is accepted**, and
must contain:

> By contributing, you agree that your contributions are licensed under the
> project's license, and you grant the project maintainer the right to
> relicense your contributions under any OSI-approved license.

Without this clause, merged contributions permanently remove the maintainer's
ability to change license, since each contributor holds copyright in their own
code. Adding the clause later does not apply retroactively.

Do not propose relicensing to MIT, Apache-2.0, or any permissive license.
Do not propose BUSL, SSPL, or Elastic License — they are not OSI-approved.

## 9. Conventions

- Rust version pinned in `rust-toolchain.toml`.
- `cargo test --workspace` and the JS test suite both green before any merge.
- No `unwrap()` in `core` outside tests.
- Public repository from day one, including while it is empty.
- README leads with a live demo link, then a GIF, then everything else.

GitHub description:

> Notation editor that stores scale degrees, not pitches — standard staff and
> numbered notation (jianpu/简谱) from one source.

GitHub topics: `music-notation`, `jianpu`, `numbered-notation`, `simplified-notation`,
`music-theory`, `rust`, `wasm`, `tauri`.
