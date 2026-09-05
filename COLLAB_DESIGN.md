# taino-edit `collab` — design notes (pre-implementation)

> Status: **draft for review — no implementation started.** This document
> resolves the architecture before any code is written, in the same
> design-before-code spirit as [DESIGN_NOTES.md](DESIGN_NOTES.md). The
> "Open questions" in §8 must be answered (and recorded here) before the
> first collab PR.

Collaborative editing — multiple peers editing one document, offline-capable,
converging without a central authority — has been on the roadmap since v0.1
as "`loro` integration behind a `collab` feature". The groundwork was laid
deliberately: every [`Step`](crates/taino-edit-core/src/step.rs) applies,
inverts, maps, and round-trips through JSON, and the trait was documented as
CRDT-extensible without reshaping. This document designs the actual bridge.

## 1. Goal and non-goals

**Goal.** Two or more peers edit the same document; each sees their own
edits immediately; payloads exchanged through *any* byte transport the
application provides; all peers converge to the same document, including
after offline periods. Rich text is first-class: concurrent bold/italic
over overlapping ranges must merge with Peritext semantics, not last-writer-wins.

**Non-goals (v1 of `collab`).**

- **No transport.** We produce and consume `Vec<u8>` payloads; websockets,
  WebRTC, polling, or carrier pigeons are the application's business.
- **No presence/awareness.** Remote cursors, names, colors — explicitly
  community surface (see ROADMAP "Out of scope"). The design must not
  *preclude* them (loro has ephemeral stores), but v1 ships without.
- **No server reference implementation.** A relay example may come later;
  the `headless-core` example pattern (two in-process peers) is enough to
  demo and test convergence.
- **No comments / suggestions / track-changes.**

## 2. Why loro

[`loro`](https://crates.io/crates/loro) (1.13.x, actively maintained, pure
Rust, wasm-ready) implements **Peritext**-style rich-text CRDT semantics —
the published algorithm for intent-preserving concurrent formatting — plus
movable-tree support (Kleppmann et al.'s move algorithm) for block
structure, update/snapshot export, and a version system (frontiers /
version vectors) we can anchor sync onto. The alternatives considered:

| Option | Verdict |
| --- | --- |
| `yrs` (Yjs port) | Mature, but rich-text marks follow Yjs semantics (weaker intent preservation for overlapping format runs than Peritext); tree support is emulated |
| `automerge` | Solid, but rich-text arrived later and its Rust API for marks is less direct; heavier payloads |
| Hand-rolled OT on our `Step`s | We already have invert/map — but correct multi-peer OT with tombstones/undo is a research project, not a roadmap item |
| **`loro`** | **Peritext semantics natively, movable tree, pure Rust, one dep** |

## 3. Architecture: where does truth live?

Three options were considered:

- **(A) loro as the single source of truth** — the taino `Node` tree becomes
  a projection of a `LoroDoc`; every command edits loro directly. Rejected:
  it rewrites the heart of core (state/transform/history all assume our
  immutable tree) and couples every consumer to loro even when `collab` is
  off. Violates the scope budget and the "core is sacred" rule.
- **(B) Dual model, op-level bridge** — the `EditorState` remains the local
  source of truth exactly as today; a `CollabSession` owns a shadow
  `LoroDoc`. Local `Step`s are translated to loro ops as they are applied;
  remote loro diffs are translated back into `Step`s and applied through the
  normal transaction pipeline (so history, selection mapping, plugins, and
  both adapters see remote edits as ordinary — but history-exempt —
  transforms). Convergence authority is loro's merge; we never rebase remote
  ops ourselves.
- **(C) Snapshot sync** — serialize the whole doc into loro on every change.
  Rejected: destroys editing intent (a concurrent bold + typing session
  degenerates into whole-document conflicts), defeating the point of
  Peritext.

**Decision: (B).** It preserves every existing invariant (adapters, history,
plugins untouched), keeps loro behind the feature gate, and matches how
ProseMirror integrates CRDTs in practice. Its cost is the translation layer,
which is exactly what §4 specifies.

## 4. The mapping: taino model ↔ loro containers

- **Block structure → `LoroTree`.** One tree node per block node (paragraph,
  heading, blockquote, list, list item, table/row/cell…). Node metadata
  (type name + attrs) in the tree node's associated map. Splits/joins/wraps
  become create/move/delete tree ops — the movable-tree algorithm gives us
  convergent structural edits (two peers moving the same list item don't
  duplicate it).
- **Textblock content → one `LoroText` per textblock**, stored under the
  block's tree node. Marks (bold, em, link…) map to Peritext mark ranges
  (`mark`/`unmark` with the mark's attrs). Inline atoms (image) are embeds
  in the text sequence.
- **Positions.** taino positions are character-indexed across the tree;
  loro text ops are per-container. The bridge maintains a block-id ↔
  tree-node-id index and translates `(doc position)` ↔ `(container, offset)`
  at the step boundary. Encoding: **Unicode scalar values** on both sides
  (loro supports it; our `Node::text` is `String` iterated by `char`) — no
  UTF-16 anywhere.

**Step translation table (the contract to implement):**

| taino `Step` | loro op(s) |
| --- | --- |
| `Replace` within one textblock | `LoroText::delete` + `insert` |
| `Replace` across blocks (split/join/delete range) | decomposed: text edits + `LoroTree` create/move/delete |
| `ReplaceAround` (wrap/lift) | `LoroTree` moves (children re-parented), wrapper create/delete |
| `AddMark` / `RemoveMark` | `LoroText::mark` / `unmark` over the range per affected textblock |
| `Attr` | tree-node map `insert` |

The inverse direction (loro diff → `Step`s) consumes loro's event/diff
stream per container and emits the corresponding steps against the current
local doc, applied in one history-exempt transaction.

## 5. History, undo, and `map_against`

- **Remote edits never enter local undo history** (transaction marked
  `no_history`, like selection mirrors today).
- **Local undo in a collab session must undo *my* edits only.** Two viable
  routes: (a) keep our `History` and, on undo, translate the inverted steps
  through the bridge like any local edit; (b) adopt loro's `UndoManager`
  (built for exactly this) and derive steps from its diffs. **Leaning (a)**
  — keeps one history implementation and our tested selection restore —
  but this is Open Question Q3.
- **`map_against(&Step)`** (the contract documented on the trait since
  Phase 2) is *not* needed for convergence under (B) — loro merges; we
  don't rebase. It remains the escape hatch for future OT-style features
  (e.g. suggestion mode) and is **out of scope for collab v1**. The design
  validates the Phase-2 bet differently than expected: the trait needed no
  reshaping, but the winning integration didn't need the method either.

## 6. Surface: a new crate, not a core feature

The v0.1 plan reserved a `collab` *feature on core*. Amended proposal:
ship **`taino-edit-collab`** as its own crate (8th in the family), because:

1. **core stays honest.** The "core is sacred" rule is about dependency
   discipline; a heavy CRDT engine behind a default-off feature still lands
   in core's dependency tree, docs, and MSRV surface. A crate isolates it
   completely (loro's MSRV moves faster than our 1.80 pin — Q4).
2. **It proves the public API.** The bridge must work through core's public
   surface (steps, transactions, schema) — the same constraint community
   extensions live under. If the bridge needs private hooks, that's core
   API feedback, not an excuse for `#[cfg]` tunnels.
3. **Versioning freedom.** loro majors can bump `taino-edit-collab`
   without touching core's version stream.

The umbrella crate re-exports it behind the (already reserved) `collab`
feature, so for users the original promise holds: 
`taino-edit = { version = "…", features = ["collab"] }`.

**API sketch (v1):**

```rust
let mut session = CollabSession::new(&schema, peer_id)?;         // fresh…
let mut session = CollabSession::from_snapshot(&schema, bytes)?; // …or joining

// Local edit path (the adapter/apply loop calls this after each commit):
session.apply_local(&transform)?;

// Outbound: bytes since the last export (or since a peer's version).
let payload: Vec<u8> = session.export_updates();

// Inbound: returns the steps to run through a no-history transaction.
let remote: Vec<Transform> = session.import_updates(&payload)?;

// Durability:
let snapshot: Vec<u8> = session.export_snapshot();
```

Everything is sans-transport and sans-async: the application decides when
to export, how to ship bytes, and when to import. Leptos/Dioxus glue
(folding `import_updates` results into the state signal) is a ~20-line
example, not adapter API.

## 7. Testing strategy

Convergence bugs are timing bugs; the strategy is **deterministic host-side
simulation** — no browser required, which our layering makes possible
(core + collab are DOM-free):

1. **Two-peer scripted scenarios** — concurrent overlapping bold+italic,
   concurrent split at the same position, type-into-deleted-block, offline
   batch then merge. Each asserts byte-identical `Node` trees after full
   exchange, *and* schema validity (`content` expressions still satisfied).
2. **Randomized convergence (property) tests** — N peers, seeded RNG,
   random step generator (reusing the schema fixtures), random partition/
   delivery orders; invariant: all peers equal after quiescence. Seeds
   printed on failure for deterministic replay.
3. **Round-trip unit tests per table row in §4** — step → loro → diff →
   step, asserting the reconstructed step applies to the same result.
4. **A `collab-headless` example** — two in-process peers over a channel,
   proving the API reads well and doubling as living documentation.

## 8. Open questions (answer before the first PR)

- **Q1 — Tree granularity for atoms/leaf blocks:** embeds in `LoroText` vs
  child tree nodes (affects image-in-paragraph and future footnotes).
- **Q2 — Initial-document seeding:** who writes the starting doc into loro
  when a session begins from existing content, and how do two peers seeding
  the "same" doc independently avoid double-insertion? (Likely: snapshot
  from a designated origin, or content-addressed seed commit.)
- **Q3 — Undo route:** our `History` (lean) vs loro `UndoManager` (§5).
- **Q4 — MSRV:** does loro 1.13 build on 1.80? If not: bump workspace MSRV
  (minor-version event) or pin an older loro.
- **Q5 — Feature name on the umbrella:** `collab` re-export only, or also
  a `collab` feature on the adapters that wires the ~20-line glue?

## 9. Effort (honest, binding once ratified)

| Phase | Scope | Estimate |
| --- | --- | --- |
| C1 | Session skeleton, tree+text mapping for paragraph-only docs, export/import, scripted 2-peer tests | ~1.2k LOC, 1–1.5 weeks |
| C2 | Marks (Peritext ranges), inline atoms, attr steps | ~0.8k LOC, ~1 week |
| C3 | Structural steps (split/join/wrap/lift, lists, tables) — the hard one | ~1.2k LOC, 1.5–2 weeks |
| C4 | Undo semantics, selection mapping for remote edits, randomized suite, example, docs | ~0.8k LOC, ~1 week |

Total: **~4k LOC and 4–5 focused weeks**, solo. Comparable to the whole
v0.1 core in risk, which is why this document exists before the code does.

## 10. References

- Peritext: *"Peritext: A CRDT for Rich-Text Collaboration"* (Litt et al.)
- Movable tree: *"A highly-available move operation for replicated trees"*
  (Kleppmann et al.)
- loro docs: <https://loro.dev> — rich text, tree, export modes, undo
- ProseMirror collab notes (op-bridge prior art): <https://prosemirror.net/docs/guide/#collab>
