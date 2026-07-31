# ssr-leptos — server-side rendering demo

The same `<TainoEditor>` as [`basic-leptos`](../basic-leptos), but running
under Leptos **SSR + hydration**: the initial document is real HTML in the
server's response — visible (and indexable) before any wasm loads — and the
wasm bundle then hydrates it into a fully live editor.

What to look at:

- `src/lib.rs` — the app. Note there is *nothing* SSR-specific around
  `<TainoEditor state=state keymap=keymap />`; the adapter handles both
  worlds. `initial_state()` builds the document the server renders.
- `tests/ssr_render.rs` — host-side proof: renders the page to a string
  (no browser) and asserts it embeds the initial document serialized
  **byte-for-byte** as `doc_view_html` emits it, and that the pre-hydration
  document is *not* `contenteditable` (read-only until the editor boots).
- `src/main.rs` — a stock `leptos_axum` server; no editor-specific code.

## Run it

With [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos)
(`cargo install cargo-leptos`):

```sh
cd examples/ssr-leptos
cargo leptos watch
```

then open <http://127.0.0.1:3000>.

Proof-of-SSR without a browser:

```sh
curl -s http://127.0.0.1:3000 | grep -o "Server-rendered with taino-edit"
```

Try also disabling JavaScript and reloading — the document still renders,
read-only.

## Tests / checks (what CI runs)

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --features ssr -- -D warnings
cargo clippy --lib --target wasm32-unknown-unknown --features hydrate -- -D warnings
cargo test --features ssr
```

## Why a separate workspace?

This example is excluded from the repo's root workspace on purpose: cargo
unifies features per build graph, and Leptos's render modes are mutually
exclusive — the root workspace's demos build Leptos with `csr`, this one
needs `ssr`/`hydrate`.
