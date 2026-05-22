# basic-dioxus

A minimal taino-edit + Dioxus demo proving the framework-agnostic
`core` / `dom` layers really do work outside Leptos.

## Run

Two ways — pick whichever toolchain you already have.

### Option A: `dx serve` (canonical Dioxus path)

```sh
# Once, if you don't have the Dioxus CLI yet:
cargo install dioxus-cli --locked

# Then, from this directory:
dx serve
```

### Option B: `trunk serve` (same toolchain as the Leptos example)

```sh
# Once, if you don't have trunk yet:
cargo install --locked trunk

# Then, from this directory:
trunk serve --open
```

Either path builds the binary to `wasm32-unknown-unknown` and serves the
demo at <http://127.0.0.1:8080>.

## Status

The Dioxus adapter now has **full event-wiring parity** with the Leptos
one: mount + incremental DOM patching, plus `input` → transform
round-trip, IME composition, sanitized paste (Markdown / HTML / text),
and `selectionchange` mirroring. Type into the editor and the live HTML
+ JSON panels track the document state.

See [`../basic-leptos`](../basic-leptos) for the toolbar-driven demo
(every extension wired to buttons).
