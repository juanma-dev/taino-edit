# basic-leptos

The full v0.1 surface in a single page: a `<TainoEditor>` plus a complete
toolbar (Bold / Italic / Paragraph / H1–H3 / Undo / Redo / Select all),
the standard `Mod-…` keymap wired on `keydown`, and a live JSON + HTML
preview of the doc so you can see the model react to every edit.

## Run

```sh
# Once, if you don't have trunk yet:
cargo install --locked trunk

# Then, from this directory:
trunk serve --open
```

Trunk builds the binary to `wasm32-unknown-unknown`, bundles it via the
`<link data-trunk rel="rust">` in `index.html`, and serves it at
<http://127.0.0.1:8080>.

## Manual test checklist

The page also embeds a `<details>` block with this same list. Each item
should produce a visible effect in the editor *and* in the live JSON /
HTML panels:

1. **Caret-only is a noop.** Click in a word and press `Mod-b` — strong
   should NOT apply (carets don't carry stored marks in v0.1).
2. **Toggle marks on a selection.** Select a word, press `Mod-b`, watch
   `<strong>` appear in both panels. Same with `Mod-i`.
3. **Change block type via shortcut.** Place the caret in a paragraph,
   press `Mod-Alt-2`. The paragraph becomes an `h2`. `Mod-Alt-0` turns
   it back.
4. **Undo / redo through the keymap.** Type some text, press `Mod-z`.
   The "Undo (Mod-z)" counter shrinks and the doc rolls back. Press
   `Mod-Shift-z` to redo.
5. **Typing reaches the state signal.** Just type — the JSON panel
   should track every character.
6. **Plain-text paste.** Copy text from any other tab and paste in.
   No markup leaks; only the characters land.
7. **HTML paste is sanitized.** Copy a fragment from a news article and
   paste it. Only the schema-known tags survive (`<p>`, `<strong>`,
   `<em>`, `h1/h2/h3`). Try `<script>` — it never reaches the doc.
8. **IME doesn't desync.** If you have a CJK / accented-input IME
   available, type something — only the committed glyphs should appear,
   never the intermediate ones (the JSON panel stays stable while you
   compose).
9. **Toolbar buttons run the same commands.** Click "Bold" — same
   effect as `Mod-b`.
10. **Mod-a selects everything.** Then `Mod-b` bolds the whole doc.

If anything misbehaves, the matching invariant is documented in
`ROADMAP.md` (Phase 4 for DOM bridge, Phase 5 for the Leptos wiring).
