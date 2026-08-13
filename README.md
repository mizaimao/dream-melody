# dream-melody

A notation editor that stores **scale degrees, not pitches** — so a score can be
rendered as standard staff notation, as numbered notation (jianpu/简谱), or as
model-ready text, from one source of truth.

Transposition moves the key and leaves the digits alone, because the digits are
what is stored. A-flat stays A-flat rather than becoming G-sharp, in every key,
including the ones that need double accidentals.

## Running it

```sh
core/wasm/build.sh              # once, and after any change to core
cargo run -p dream-melody-tauri # the desktop application
```

The interface is embedded in the executable, so the application is one file
that needs no server and no network. The wasm is generated rather than
committed, so the first step is not optional.

There is also a browser runner, useful when the developer tools are wanted:

```sh
cargo run -p dream-melody-dev
```

It serves the same interface on a loopback address and opens a browser.

## Testing

```sh
cargo test --workspace                  # the model, the formats, the runner
node --test 'core/wasm/tests/*.test.mjs' # the browser bindings
node --test 'web-tests/*.test.mjs'       # what actually reaches the page
```

The last of these drives a real headless browser and reads the drawn SVG,
because the bugs worth catching there are not arithmetic. It skips when no
Chrome is present.

## What it does so far

Everything below the interface is finished and tested: the model, transposition,
numbered notation both ways, MusicXML both ways, ABC out, MIDI both ways, and
JSON persistence. The browser draws the staff and plays it back.

See [SPEC.md](SPEC.md) for the design and the build order.

## Licence

AGPL-3.0-or-later. See [LICENSE](LICENSE).

Anyone who uses this code must publish their own — including if they only run it
as a network service. That network clause is the point.

VexFlow is vendored in `web/vendor/` under the MIT licence.
