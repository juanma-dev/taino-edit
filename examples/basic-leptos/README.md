# basic-leptos

A minimal `<TainoEditor>` demo: a single Leptos component that mounts a
contenteditable editor, plus three buttons (Bold / Undo / Redo) showing how
core commands and `EditorState::undo/redo` flow through the state signal.

## Build & run

Requires [trunk](https://trunkrs.dev/):

```sh
cd examples/basic-leptos
trunk serve --open
```

That's the Phase 5 definition of done: trunk compiles the binary to
`wasm32-unknown-unknown`, bundles it via the `<link data-trunk rel="rust">`
in `index.html`, and serves it at <http://127.0.0.1:8080>.
