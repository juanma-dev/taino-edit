# headless-core

A server-side / CLI demo of `taino-edit-core` + `taino-edit-extensions`.
Demonstrates that the editor's model, schema, transforms, JSON / HTML
round-trip and commands all work **without a browser or DOM**.

```sh
cargo run -p headless-core
```

Output (abridged):

```
after edit  → HiHello world
json bytes  → ...
html        → <h1>Hi</h1><p>Hello world</p>
after bold  → <h1>Hi</h1><p>Hello <strong>world</strong></p>
after undo  → <h1>Hi</h1><p>Hello world</p>
```

What it exercises:

- composing a schema from the v0.1 extension set on top of the universal
  `doc`/`text` primitives;
- editing through `EditorState::tr()` + `Transform::insert`;
- JSON round-trip (`Node::to_json` ↔ `Schema::node_from_json`);
- HTML serialization + strict, depth-bounded HTML parsing;
- driving a command (`toggle_mark`) and `undo` through the built keymap.

If this binary compiles and runs on a server with no display, no DOM,
no JS runtime — taino-edit-core is doing its job.
