# basic-dioxus

A minimal taino-edit + Dioxus demo proving the framework-agnostic
`core` / `dom` layers really do work outside Leptos.

## Run

```sh
# Once, if you don't have the Dioxus CLI yet:
cargo install dioxus-cli --locked

# Then, from this directory:
dx serve
```

`dx serve` builds the binary to `wasm32-unknown-unknown` and serves the
demo at <http://127.0.0.1:8080>.

## Status

v0.2 of the Dioxus adapter is a **minimum-viable adapter**: mount + DOM
patching work end-to-end so changes to the `Signal<EditorState>` are
reflected in the rendered editor. Full event-wiring parity with the
Leptos adapter (input → transform round-trip, IME composition, paste,
`selectionchange`) lands in v0.2.x — the `taino-edit-dom` pieces it
needs are already in place.

For a production-grade demo today see
[`../basic-leptos`](../basic-leptos).
