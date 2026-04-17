# Unreleased

## Phase 2o — Kill the side-table

Final structural step of Phase 2. The thread-local `HEAP_OBJECTS`
vector and its `store_heap_object` / `clear_heap_objects` helpers are
gone. `value_to_obj` no longer has a legacy side-table decode arm.
Every heap-typed `LispObject` now routes through the real GC heap via
a `HeapScope`.

### Added

- `ObjectTag::Primitive = 8` and `gc::PrimitiveObject { header, name: String }`.
- `Heap::primitive_value(name: &str) -> Value` — the last allocator
  needed to retire the side-table.
- `value_to_obj` decode arm for `ObjectTag::Primitive` returning
  `LispObject::Primitive(obj.name.clone())`.
- Test `gc::tests::primitive_allocation_and_sweep`.

### Changed

- `obj_to_value(LispObject::Primitive(name))` now routes to
  `heap.primitive_value(name)` under a `HeapScope`.
- The fallback behaviour when no `HeapScope` is installed: returns
  `Value::nil()` with a `debug_assert!(false, ...)` tripwire. In
  release builds this is a silent graceful-degradation; in debug it
  panics with a clear message ("install a HeapScope via
  `HeapScope::enter(heap)` or call through `Interpreter::eval`").
- `value_to_obj`'s `val.is_ptr()` side-table arm is removed — no
  main-interpreter code path produces `TAG_GC_PTR` any more. Values
  with that tag flow through unchanged to the final `Nil` fallback
  (shouldn't happen in the main interpreter).

### Removed

- `HEAP_OBJECTS` thread-local.
- `store_heap_object(obj)` helper.
- `clear_heap_objects()` helper (no known external callers; wasn't
  needed after Phase 2m anyway).

### Kept (VM-internal)

- `TAG_GC_PTR = 1` constant, `Value::is_ptr`, `Value::as_ptr`,
  `Value::from_ptr` — the bytecode interpreter in `vm.rs` has its
  own independent side-table (`VM::heap_objects`) that produces and
  consumes `TAG_GC_PTR` values. Migrating the VM to the real heap is
  a separate project; the tag stays until then.

### Regression guards

All green: `test_setcar_mutates_in_place`, `test_setcdr_mutates_in_place`,
`test_puthash_mutates_in_place`, `test_cons_setcar_after_heap_round_trip`,
`test_cons_setcdr_after_heap_round_trip`,
`test_hashtable_identity_preserved_under_heap_scope`.

### Updated tests

- `bridge_string_falls_back_to_side_table_when_no_scope` renamed to
  `bridge_string_without_scope_returns_nil` with `#[cfg(not(debug_assertions))]`
  guard so the assertion-tripwire doesn't fire during the test.
- `cons_value_tag_distinct_from_side_table` simplified to
  `cons_value_produces_tag_heap_ptr` — the side-table half of the
  test no longer exercises anything meaningful.

### Verification

285/286 tests pass. Same pre-existing `test_backquote_expansion`
env-dependent failure. Clippy + fmt clean.

### Phase 2 complete

All six heap types (Cons, String, Vector, HashTable, BytecodeFn,
Bignum) plus Primitive now allocate through the real GC heap with
identity preservation where it matters. The side-table is gone from
the main interpreter. Remaining work beyond Phase 2:

- Migrate the VM (`vm.rs`) to use the real heap instead of its own
  `heap_objects` side-table. Will kill the last user of `TAG_GC_PTR`.
- `obj_to_value` / `value_to_obj` simplifications now that the
  legacy path is gone (e.g. narrowing `Value::is_cons` to only
  inspect `TAG_HEAP_PTR` + the pointed-to `ObjectTag`).
- Optimisations: arena-pool for `ConsArcCell`, small-string
  optimisation, per-type mark bitmaps, generational GC, etc.

## Phase 2n-cons — Identity-preserving ConsArcCell + Cons migration

Final heap-type migration. Cons now routes through `obj_to_value`
alongside String/Vector/HashTable/BytecodeFn/Bignum. With this phase
landed, only `LispObject::Primitive` still uses the side-table, and
`HEAP_OBJECTS` is close to deletable (Phase 2o).

### Design: two cons variants

Option (b) from the 2n plan. Cons has **two** heap object layouts
living side by side — each used where its trade-off wins:

- **`ConsCell` (ObjectTag::Cons = 0)** — the existing u64 layout
  `{ car: u64, cdr: u64 }`, bump-allocated via the typed `Arena`,
  24 bytes per cell. Used by native Value-based list builders
  (`sort`, `nreverse`, `version-to-list`, `(garbage-collect)`'s
  stats plist, etc.). `value_to_obj` decodes into a **fresh**
  `LispObject::Cons(Arc::new(...))` — identity is *not* preserved
  across round-trips, which is fine because these results are
  read-only in practice.
- **`ConsArcCell` (ObjectTag::ConsArc = 7)** — new layout that
  wraps the same `Arc<Mutex<(LispObject, LispObject)>>` as
  `LispObject::Cons`. `value_to_obj` returns `Arc::clone` — mutation
  via `setcar`/`setcdr` and `(eq x x)` semantics survive the Value
  round-trip.

Choosing the right variant is automatic:

- `Heap::cons_value(car, cdr)` → `ConsCell`. Called by
  `InterpreterState::{heap_cons, with_heap(|h| h.cons_value(...))}`
  and the `list_from_*` helpers — all internal Value-based builders.
- `Heap::cons_arc_value(Arc<Mutex<_>>)` → `ConsArcCell`. Called by
  `obj_to_value(LispObject::Cons(arc))` under a `HeapScope`.

### Added

- `ObjectTag::ConsArc = 7`.
- `gc::ConsArcCell { header, arc: object::ConsCell }` struct.
- `Heap::cons_arc_value(arc) -> Value` allocator.
- New decode arm in `value_to_obj` for `ObjectTag::ConsArc`.
- Four new tests:
  - `gc::tests::cons_arc_preserves_identity_across_decode` — inspects
    the heap layer directly, asserts `Arc::ptr_eq` between the
    caller's Arc and the heap object's Arc, and verifies mutation
    through one is visible via the other.
  - `gc::tests::cons_arc_unrooted_swept`.
  - `eval::tests::test_cons_setcar_after_heap_round_trip` — end-to-end
    `(let ((x (cons 'a 'b))) (setcar x 'z) (car x))` returns `'z`.
  - `eval::tests::test_cons_setcdr_after_heap_round_trip` — the same
    for `setcdr`.

### Changed

- `obj_to_value(LispObject::Cons(arc))` now routes to
  `with_current_heap(|h| h.cons_arc_value(arc.clone()))`, falling
  back to the side-table when no `HeapScope` is active.
- `Heap::mark_object` / `sweep` / `object_size` each gain a
  `ConsArc` arm. `mark_object` treats it as a leaf (Arc refcount
  handles inner lifetimes); `sweep` reconstructs the Box via
  `Box::from_raw` to drop the Arc; `object_size` returns
  `size_of::<ConsArcCell>()`.

### Regression guards

- `test_setcar_mutates_in_place` and `test_setcdr_mutates_in_place`
  on quoted lists — still green.
- `test_puthash_mutates_in_place` — still green.

### Verification

284/285 tests pass. Same pre-existing `test_backquote_expansion`
env-dependent failure.

### What's left of Phase 2

Only Phase 2o remains: remove `HEAP_OBJECTS`, `store_heap_object`,
`clear_heap_objects`, the side-table arm of `obj_to_value`
(currently only used for `LispObject::Primitive` and the fallback
path when no `HeapScope` is installed), and `TAG_GC_PTR`.
`LispObject::Primitive` gets a small dedicated heap object type or
a NaN-box tag of its own. After 2o, `value_to_obj` is pure NaN-box
decoding with no thread-local state.

## Phase 2n — Arc-wrapping heap objects for mutable containers + three more migrations

Phase 2m migrated `obj_to_value` for immutable heap types only
(String, Bignum). Phase 2n redesigns the mutable container heap
object layouts to preserve `Arc` identity across round-trips, then
migrates `Vector`, `HashTable`, and `BytecodeFn` through
`obj_to_value` as well.

### Design change

`VectorObject` and `HashTableObject` previously owned cloned content:
`VectorObject { elements: Box<[u64]> }` (raw Value bits) and
`HashTableObject { table: LispHashTable }` (owned map). Every
`obj_to_value` call would have produced a fresh heap object with
cloned data, and `value_to_obj` would have returned yet another
clone — breaking `puthash`→`gethash` mutation-visibility and
`(eq x x)` on mutable containers.

Phase 2n redesigns both to wrap the existing `Arc<Mutex<_>>`
containers that `LispObject::Vector` / `LispObject::HashTable`
already use:

- `VectorObject { header, v: SharedVec }`
  where `SharedVec = Arc<Mutex<Vec<LispObject>>>`.
- `HashTableObject { header, table: SharedHashTable }`
  where `SharedHashTable = Arc<Mutex<LispHashTable>>`.

Every `obj_to_value` call for these types now creates a new heap
object **sharing the caller's Arc**. `value_to_obj` decodes via
`Arc::clone`. Mutations propagate through the shared Arc regardless
of how many heap objects or LispObject references exist at the
moment.

### Added

- Three new regression tests:
  `test_hashtable_identity_preserved_under_heap_scope`,
  `test_vector_decode_preserves_content`,
  `test_hashtable_puthash_persists_across_rebindings`.

### Changed

- `gc::VectorObject` field renamed `elements: Box<[u64]>` → `v:
  SharedVec`.
- `gc::HashTableObject` field typed as `SharedHashTable` instead of
  owned `LispHashTable`.
- `Heap::vector_value(elements: &[Value])` → `vector_value(v:
  SharedVec)`. Caller owns the Arc.
- `Heap::hashtable_value(table: LispHashTable)` → `hashtable_value(table:
  SharedHashTable)`.
- `Heap::mark_object`: Vector / HashTable / ByteCode arms collapse to
  a single no-op arm. Element lifetimes are governed by `Arc`
  refcounting, not mark-sweep — tracing has nothing to do.
- `Heap::object_size` for Vector now returns
  `size_of::<VectorObject>()` (the inner `Vec<LispObject>` lives
  behind the Arc, allocator-managed, not attributed to the GC heap).
- `value_to_obj`: Vector / HashTable arms return
  `LispObject::{Vector, HashTable}(obj.{v,table}.clone())` — Arc
  clone preserves identity.
- `obj_to_value`: routes `LispObject::{Vector, HashTable, BytecodeFn}`
  through the real heap via the Phase-2m `HeapScope`, falling back to
  the side-table when no scope is active. Cons / Primitive remain on
  the side-table (their redesign is Phase 2n-cons).
- `InterpreterState::heap_vector`, `heap_vector_from_objects`,
  `heap_hashtable` updated to build a fresh Arc and hand it to the
  new allocator signatures. `heap_vector` also eagerly collects
  elements via `value_to_obj` BEFORE locking the heap to avoid the
  same reentrant-lock hazard that bit earlier phases.

### Regression guards

- `test_setcar_mutates_in_place` — still passing. (Cons stays on the
  side-table so this path is unchanged.)
- `test_puthash_mutates_in_place` — still passing. (Now actually
  exercising the real heap under Phase 2n + 2m.)
- `test_hashtable_puthash_persists_across_rebindings` — new test.

### Verification

280/281 tests pass. The one failure is the pre-existing
`test_backquote_expansion` env-dependent path bug.

### Deferred

- **Cons redesign (Phase 2n-cons)**: `ConsCell { car: u64, cdr: u64 }`
  doesn't wrap an Arc, so `obj_to_value(LispObject::Cons(arc))` would
  break setcar/setcdr identity. Migration needs a Cons layout
  redesign (wrap Arc, or add a separate `ConsArcCell` variant) — a
  real trade-off because the u64 layout has Value-bit advantages for
  cons chains built natively from Values (Phase 2c–2e uses).
- **Phase 2o**: remove the `HEAP_OBJECTS` side-table once Cons is
  migrated.

## Phase 2m — Heap-aware `obj_to_value` via thread-local scope (String + Bignum)

First step toward killing the `HEAP_OBJECTS` side-table. `obj_to_value`
now routes **identity-safe** heap types (String, out-of-range Integer)
through the interpreter's real GC heap when one is installed, instead
of the legacy side-table. Identity-sensitive types (Cons, Vector,
HashTable, BytecodeFn) continue to use the side-table — migrating them
would break `(eq mutable-x mutable-x)` and in-place mutation semantics
until their heap object layouts wrap `Arc<Mutex<_>>` for identity
preservation (Phase 2n).

### Added

- `value::HeapScope` RAII guard with `HeapScope::enter(Arc<Mutex<Heap>>)`.
  Nested scopes stack (LIFO restore on drop) so reentrant
  `Interpreter::eval` calls from hooks are safe.
- `value::CURRENT_HEAP` thread-local + private `with_current_heap<R>(F)`
  helper. Clones the `Arc` out of the `RefCell` before locking the
  `Mutex` so neither borrow is held across the lock.
- Three new bridge tests:
  `bridge_string_routes_to_heap_when_scope_active`,
  `bridge_string_falls_back_to_side_table_when_no_scope`,
  `bridge_bignum_routes_to_heap_when_scope_active`.

### Changed

- `obj_to_value`:
  - `LispObject::String(s)` → `with_current_heap(|h| h.string_value(s))`,
    falling back to `store_heap_object(obj)` when no scope is active.
  - `LispObject::Integer(n)` outside the 48-bit fixnum range →
    `with_current_heap(|h| h.bignum_value(*n))` with the same fallback.
  - `Cons`/`Vector`/`HashTable`/`BytecodeFn`/`Primitive` unchanged —
    docstring explains the identity constraint and the 2n plan.
- `Interpreter::eval`, `Interpreter::eval_value`, and
  `Interpreter::eval_source_value` install a `HeapScope` covering their
  invocation. `eval_source` delegates to `eval` per form, so each
  form gets its own scope (wasteful but correct).
- **Deadlock fix**: `list_from_objects` and
  `list_from_objects_reversed` previously called `obj_to_value`
  *inside* their `with_heap` closure. With Phase 2m that becomes a
  reentrant lock on `parking_lot::Mutex` (which is not reentrant) and
  hangs the test suite. Fix: eager `.collect()` of items to Values
  BEFORE entering `with_heap`, matching the same pattern already used
  in `split-string` from Phase 2g. Inline comment documents the
  hazard.

### Regression guard

`test_setcar_mutates_in_place` and `test_puthash_mutates_in_place`
still pass — proof that the mutable-container identity semantics
survive 2m because those types are explicitly NOT routed through the
heap yet.

### Verification

277/278 tests pass. The single failure is the pre-existing
`test_backquote_expansion` env-dependent path bug (looks in
`/tmp/elisp-stdlib/`, files live in `/tmp/elisp-stdlib/emacs-lisp/`).
Clippy + fmt clean.

### Deferred to 2n

Redesign the Cons/Vector/HashTable/BytecodeFn heap object layouts to
wrap the existing `Arc<Mutex<_>>` containers instead of owning content.
Then `obj_to_value` can route those types through the heap while
preserving mutation and `eq` identity.

## Phase 2l — heap_vector / heap_hashtable chokepoints + 4 more migrations

Mechanical migration of non-cons heap construction sites in
`eval/mod.rs` away from `obj_to_value` → side-table.

### Added

- `InterpreterState::heap_vector<I>(I: IntoIterator<Item=Value>)`:
  collects values and allocates a VectorObject on the heap.
- `InterpreterState::heap_vector_from_objects(&[LispObject])`:
  convenience for sites with `Vec<LispObject>` already in hand.
  Items are converted via `obj_to_value` before being stored as raw
  Value bits.
- `InterpreterState::heap_hashtable(LispHashTable)`: wraps an
  existing hash table in a HashTableObject on the heap.

### Changed

- `make-hash-table` primitive: returns a real-heap hash table via
  `state.heap_hashtable(...)` instead of the side-table round-trip.
- `vector` primitive (Vec→Vector builder): uses
  `state.heap_vector_from_objects(&items)`.
- `make-vector` primitive: same.
- `vconcat` primitive: same.

### Bug fix (caught during verification)

- **Mutex deadlock in Phase 2g split-string migration**: the lazy
  `parts.into_iter().map(|p| state.heap_string(&p))` passed to
  `list_from_values` tried to acquire the heap mutex inside the
  `with_heap` closure (which already holds it). `parking_lot::Mutex`
  is not reentrant — the test suite hung. Fix: eager `.collect()` of
  the element values BEFORE calling `list_from_values`. Comment
  block in-source explains the hazard for future migrations.

### Remaining

- `LispObject::BytecodeFn(...)` construction sites would follow the
  same pattern but the current codebase doesn't produce bytecode
  from eval — bytecode comes in via the reader parsing `.elc` files.
  Revisit when the reader itself migrates.
- Oversized-integer (`|n| > 2^47`) fallback in `obj_to_value` still
  routes through the side-table. Migrating requires
  `obj_to_value` to have heap access — structural change deferred.
- Removing the `HEAP_OBJECTS` side-table entirely (Phase 2o):
  requires `obj_to_value` to become a method on `InterpreterState`
  (or a thread-local heap handle). Touches many call sites;
  deliberately deferred to stay under the 5-file cap per sub-phase.

## Phase 2g — `heap_string` chokepoint + 2 site migrations

First site migrations that route strings through the real GC heap
instead of the `HEAP_OBJECTS` side-table.

### Added

- `InterpreterState::heap_string(s: &str) -> Value` — chokepoint for
  allocating strings on the real GC heap via `Heap::string_value`.
- `InterpreterState::list_from_values<I>(values: I) -> Value` —
  mirror of `list_from_objects` but takes `Value`s directly. Used
  when call sites already produce heap-allocated Values.

### Changed

- `buffer-list` primitive: `"*scratch*"` now allocated via
  `state.heap_string(...)` and the surrounding cons via
  `state.list_from_values(...)`.
- `split-string` primitive: each part allocated via
  `state.heap_string(...)`; list spine built from the resulting
  Values.

## Phase 2h/2i/2j/2k — Heap allocators for Vector, HashTable, BytecodeFn, Bignum

Infrastructure batch. Four new heap object types, all following the
same shape established in Phase 2f (String): `#[repr(C)]` struct with
`GcHeader` prefix, `Box::into_raw` allocation, `mark_object` arm,
`sweep` arm via `Box::from_raw`, `object_size` arm, and `value_to_obj`
decode arm. No call-site migrations in this batch — each allocator is
plumbed end-to-end but not yet used by the interpreter.

### Added

- **Vector** (`VectorObject`): `Box<[u64]>` element storage where each
  slot holds a `Value::raw()` bit pattern. `Heap::vector_value(&[Value])`
  allocator. `mark_object` decodes each slot and traces heap pointers
  (this is the first non-Cons heap type with child references).
- **HashTable** (`HashTableObject`): wraps existing `LispHashTable` so
  its `HashMap<HashKey, LispObject>` internals are unchanged. No child
  tracing yet because contents are `LispObject`, not `Value`.
- **BytecodeFn** (`BytecodeFnObject`): wraps existing
  `BytecodeFunction`. Same no-child-tracing rationale.
- **Bignum** (`BignumObject`): stores `i64` for integers outside the
  48-bit fixnum range. Placeholder until arbitrary-precision is
  needed; current codebase never exceeds `i64::MAX`.
- Five new GC tests: `vector_allocation_and_tracing`,
  `vector_unrooted_swept`, `hashtable_allocation_and_sweep`,
  `bytecode_allocation_and_sweep`, `bignum_allocation_rooted_survives`.

### Changed

- `Heap::mark_object`: exhaustive match on `GcHeader.tag`. Cons traces
  car/cdr; Vector traces each element. String/HashTable/ByteCode/
  Bignum/Symbol are leaves for now.
- `Heap::sweep`: exhaustive match frees via `Box::from_raw` for every
  Box-allocated tag; Cons goes to its arena free list; Symbol is
  unlinked only (externally registered).
- `Heap::object_size`: returns accurate byte accounting for every tag
  so the GC threshold sees real memory pressure.
- `value_to_obj`: dispatches on `GcHeader.tag` and materialises the
  appropriate legacy `LispObject` (Vector → `Arc<Mutex<Vec<_>>>`,
  HashTable → `Arc<Mutex<LispHashTable>>`, ByteCode → cloned
  `BytecodeFunction`, Bignum → `Integer(i64)`).

### Still deferred

- No migration of interpreter call sites to use the new allocators —
  that's Phase 2l+ with one phase per container type to stay under
  the 5-file cap.
- Vectors trace their elements, but hash tables and bytecode don't
  yet — their internals still use `LispObject`. Tracing gets wired
  up when those containers move to `Value`-keyed storage.
- Removing the `HEAP_OBJECTS` side-table. Needs all call sites
  migrated first.

## Phase 2f — String objects on the real GC heap

Pure infrastructure sub-phase. `Heap::string_value` now allocates real
GC-managed `StringObject`s — strings are the first non-cons heap type
to live outside the `HEAP_OBJECTS` side-table. No interpreter call
site is migrated yet; that's Phase 2g. This phase proves the
tagged-heap-object pattern (single `TAG_HEAP_PTR` Value tag, object
type discriminated by `GcHeader.tag` on the pointed-to header)
generalises from cons to variable-length leaf objects.

### Added

- `gc::StringObject` — variable-length heap object with
  `GcHeader` prefix and owned `Box<str>` payload.
- `Heap::string_value(s: &str) -> Value` — allocator. Individually
  `Box::new`-allocated, linked into `all_objects`, returns a
  `TAG_HEAP_PTR` Value.
- `value_to_obj` decode arm for `ObjectTag::String`: reads the
  `Box<str>` and materialises a `LispObject::String`.
- Four new GC tests (`string_allocation_basic`,
  `string_unrooted_swept`, `string_rooted_survives_gc`,
  `cons_containing_string_survives_gc_rooted`) and one bridge test
  (`bridge_heap_string_decodes_via_value_to_obj`).

### Changed

- `Heap::mark_object` now dispatches on `GcHeader.tag` with a match;
  `String` is a leaf (no children to trace), `Cons` behaviour is
  unchanged.
- `Heap::sweep` reclaims `StringObject` via `Box::from_raw`, running
  the `Box<str>` destructor to return memory to the global allocator.
- `Heap::object_size` accounts for `size_of::<StringObject>() +
  data.len()` for string objects, so the GC threshold correctly sees
  large-string pressure.

### Design notes

- **Encoding**: still a single `TAG_HEAP_PTR = 6` Value tag for all
  heap objects. Discrimination by `GcHeader.tag` reads one cache line
  per decode. Value's tag space is precious — we have 7 tags used out
  of 8, reserving the last one for something that actually needs a
  distinct tag (likely Bignum, where the sign bit matters).
- **Allocation**: strings don't fit in a typed arena (variable-size),
  so each gets its own `Box::into_raw` allocation. `StringObject` is
  `#[repr(C)]` with `header` at offset zero so the cast
  `*mut GcHeader ↔ *mut StringObject` is always valid.
- **No write barriers needed**: strings are immutable leaves. Mutation
  of Lisp strings (`aset`) goes through the LispObject path and does
  not write into heap-allocated `Box<str>` — that stays a compile-time
  concern for later.

### Still deferred

- Vector / HashTable / BytecodeFn / Bignum allocators. Each needs the
  same shape: struct, allocator method, mark arm, sweep arm, decode
  arm. Vectors and hash tables additionally need child tracing.
- Migrating `LispObject::String(_)` call sites in the interpreter to
  `state.heap_string(...)`. That's Phase 2g.
- Removing the `HEAP_OBJECTS` side-table.

## Phase 2e — Reversed-list helper + four more site migrations

Phase 2e adds the mirror of Phase 2d's helper (`list_from_objects_reversed`
for the nreverse-shape pattern) and migrates four more cons
construction sites in `eval/mod.rs`.

### Added

- `InterpreterState::list_from_objects_reversed<I>(items: I) -> Value`:
  iterates items in natural order and prepends each — `[a, b, c]`
  produces `(c b a)`. Used by `nreverse`. Complements the Phase 2d
  `list_from_objects` which produces natural-order lists.
- Five new integration tests:
  `test_nreverse_heap_migration`, `test_nreverse_empty_list`,
  `test_split_string_heap_migration`,
  `test_read_from_string_heap_migration`,
  `test_symbol_function_macro_form_heap_migration`.

### Changed

- `nreverse` primitive: replaced manual Vec→reversed-list fold with
  `state.list_from_objects_reversed(items)`.
- `split-string` primitive: replaced manual Vec<String>→list fold with
  `state.list_from_objects(parts.into_iter().map(LispObject::string))`.
- `read-from-string` primitive: the `(obj . end_pos)` dotted pair is
  now built via `state.heap_cons(obj_val, end_val)` directly.
- `symbol-function` primitive: the `(macro lambda ARGS . BODY)` wrapper
  returned for macros is now built as a 3-cell cons chain under a
  single `with_heap` closure.

### Not migrated (deliberately)

- `sort` predicate call-args (eval/mod.rs:1170) — inside the
  comparator, called O(n log n) times per sort. Each allocation would
  be heap garbage until the next explicit `(garbage-collect)`. Not a
  net win without proper roots/GC coordination.
- `mapconcat` per-iteration call-args (eval/mod.rs:1652) — same hot-loop
  concern.
- Signal `data` field (eval/mod.rs:1571) — `SignalData::data` is typed
  `LispObject`, so the cons has to stay in that representation until
  `SignalData` itself moves to Value. Tracked separately.
- `push` / `pop` (eval/mod.rs:1040) — cons is written back into the
  env via `env.set` which still expects `LispObject`. Needs env-layer
  refactor first.

### Still deferred

- Primitives in `primitives.rs` and VM opcodes in `vm.rs`.
- Reader migration.
- Extending `Heap` to allocate String/Vector/HashTable/BytecodeFn.
- Removing the `HEAP_OBJECTS` side-table.

## Phase 2d — Vec→list helper + three site migrations

Phase 2c proved `Heap::cons_value` works end-to-end inside the
interpreter. Phase 2d extracts the common "build a Lisp list from an
in-memory Vec" shape into a reusable helper and migrates three sites
that match it exactly. Every future Vec→list migration is now a
one-liner.

### Added

- `InterpreterState::list_from_objects<I>(items: I) -> Value` where
  `I: IntoIterator<Item = LispObject>, I::IntoIter: DoubleEndedIterator`.
  Builds a proper list on the real GC heap by iterating the items in
  reverse and prepending each — all under one `with_heap` closure, so
  the heap mutex is taken exactly once per call. Each `LispObject` goes
  through the existing `obj_to_value` bridge, so immediates stay
  immediate and heap-typed items fall back to the side-table. Only the
  list *spine* (the cons cells themselves) lives on the real GC heap.
- Three new integration tests in `eval/tests.rs`:
  `test_version_to_list_heap_migration`,
  `test_version_to_list_empty_parts`,
  `test_sort_ascending_heap_migration`. `test_buffer_list` already
  covered the buffer-list migration.

### Changed

- `sort` primitive: replaced the manual `LispObject::nil()` +
  `LispObject::cons` fold over `items.into_iter().rev()` with a single
  `state.list_from_objects(items)` call.
- `version-to-list` primitive: similar — parse parts into
  `Vec<LispObject>` and hand to `list_from_objects`.
- `buffer-list` primitive: the single-cell cons `("*scratch*" . nil)`
  is now built via `state.list_from_objects(std::iter::once(...))`.

### Still deferred

- `nreverse` (`eval/mod.rs:1226-1230`) — same Vec→list shape but
  iterates forward (that's the trick that reverses the list). Wants a
  direction-aware variant of the helper. Phase 2e candidate.
- Other `eval/mod.rs` sites (push, macro call-args, error-data
  construction, path list construction) — each crosses a semantic
  boundary that needs dedicated work.
- Primitives in `primitives.rs` and VM opcodes in `vm.rs` — still
  LispObject-shaped end-to-end; need a type-level refactor first.
- Reader migration — still deferred.

## Phase 2c — First real cons migration: `(garbage-collect)` result plist

The `(garbage-collect)` primitive now builds its stats alist on the
real GC heap via the Phase-2b `InterpreterState::with_heap` chokepoint.
Six cons cells per call — the `(bytes-allocated . N) (gc-count . N)
(cons-total . N)` alist — are allocated through `Heap::cons_value`
under a single lock acquisition and returned as a `TAG_HEAP_PTR` Value.
`value_to_obj` decodes the chain back into `LispObject::Cons` at the
eval boundary, so `Interpreter::eval`'s return type and the two
existing tests (`test_garbage_collect_returns_stats`,
`test_garbage_collect_reports_cons_total`) are unchanged.

### Why this is safe

- The primitive itself runs `heap.collect()` as its first step, so it
  allocates on a freshly-swept heap; nothing to root from prior state.
- Allocation happens under a single `with_heap` closure — no mutex
  contention, no intermediate safepoints within the construction.
- The interpreter runs in `GcMode::Manual`, so no other heap allocation
  site sweeps implicitly. The returned Value is safe until the next
  explicit `(garbage-collect)` call.
- Between primitive return and `Interpreter::eval`'s `value_to_obj`,
  no code path triggers GC. `value_to_obj` reads the heap cells and
  materialises `LispObject::Cons` copies, which are independent of the
  heap afterwards.

### Changed

- `(garbage-collect)` primitive (`eval/mod.rs`) no longer constructs
  its result via `LispObject::cons`. Symbols are interned outside the
  heap lock; all six cons allocations happen inside one `with_heap`
  closure.

### Still deferred

- `LispObject::cons` is still in use everywhere else. Next candidates
  by ease:
  1. Eval/VM internal helpers whose cons output is consumed in the
     same scope (no LispObject boundary crossed).
  2. `prim_cons` and cons-building primitives in `primitives.rs` —
     these take `&LispObject` args and return `LispObject`, so they
     need an eval-dispatch redesign or an intermediate conversion
     layer first.
  3. Reader (`reader.rs`, 35 sites) — revisit last, after the
     VM/primitives move to Value-first.
- Automatic safepoint insertion in the eval loop.

## Phase 2b — Safepoint model + chokepoint for future cons migration

No call-site migration in this sub-phase. Lands the safety infrastructure
that every Phase-2c-and-beyond migration depends on, and preempts the
rooting-bug class that Phase 2a's `cons_chain_rooted_survives_gc` test
surfaced on its first run.

### Added

- `gc::GcMode` enum (`Auto`, `Manual`) and `Heap::set_gc_mode` /
  `Heap::gc_mode` accessors. `Auto` preserves the current
  threshold-driven sweeping behaviour; `Manual` disables implicit
  sweeps inside `maybe_gc` so only explicit `Heap::collect()` runs the
  GC.
- `Heap::root_value(Value) -> Option<usize>`: Value-aware rooting
  helper. Returns the root-stack index for pinning real heap pointers
  (`TAG_HEAP_PTR`) and `None` for immediates / side-table indices that
  don't need rooting. Indexes are popped via the existing
  `Heap::pop_root(idx)`.
- `InterpreterState::heap_cons(car, cdr) -> Value`: single chokepoint
  for all future cons-construction migration. Every site that moves
  off `LispObject::cons` goes through this method.
- `InterpreterState::with_heap<F, R>`: one-lock helper for multi-step
  heap flows (allocate several cells, push/pop roots, collect).
- Four new GC tests in `gc.rs`:
  `gc_mode_manual_suppresses_automatic_sweeps`,
  `gc_mode_auto_still_sweeps_past_threshold`,
  `root_value_returns_none_for_immediates`,
  `root_value_keeps_heap_cons_alive_across_gc`.

### Changed

- `Interpreter::new` constructs its `Heap` in `GcMode::Manual`. Safe
  today because nothing allocates on the real heap yet — the only
  sweep site is the explicit `(garbage-collect)` primitive.
- `Heap::maybe_gc` no-ops when `gc_mode == Manual`.

### Deferred

- The actual call-site migration (`LispObject::cons` →
  `InterpreterState::heap_cons`). Phase 2c, recommended starter:
  `(garbage-collect)` primitive result-building (eval/mod.rs:687-705,
  4 nested cons, self-contained, called at a natural safepoint).
- Reader migration is NOT a good target — its public
  `ElispResult<LispObject>` signature has 100+ test callers.
- Automatic safepoint insertion in the eval loop.

## Phase 2a — Traceable heap cons cells (distinct from the side-table tag)

Prerequisite for the full GC migration. Makes `Heap::cons_value` actually
usable: until now it returned a Value tagged `TAG_GC_PTR` (= 1), the same
tag used by the thread-local `HEAP_OBJECTS` side-table for legacy
`LispObject` storage. The collision meant any code path that called
`cons_value` would later mis-decode the raw pointer as a side-table
index.

No interpreter call site is migrated in this sub-phase — every cons
construction still goes through `LispObject::cons` and the side-table.
This patch only lays the groundwork.

### Added

- `TAG_HEAP_PTR = 6` in `value.rs`: distinct NaN-box tag for real
  `*mut GcHeader` pointers allocated by `Heap`.
- `Value::heap_ptr`, `Value::is_heap_ptr`, `Value::as_heap_ptr`:
  constructor and predicates for the new tag.
- `Value::trace(visit)`: mark-phase helper that invokes `visit` for each
  `TAG_HEAP_PTR` the Value holds (and nothing for immediates or
  side-table indices).
- `value_to_obj` now decodes `TAG_HEAP_PTR` cons cells recursively into
  a legacy `LispObject::Cons`, keeping the legacy API usable against
  heap-allocated cons during the transition.
- Four new GC tests in `gc.rs`:
  `cons_chain_rooted_survives_gc`, `unrooted_cons_chain_swept`,
  `cycle_is_collected_when_unrooted`,
  `cons_value_tag_distinct_from_side_table`.

### Changed

- `Heap::cons_value` now returns a `Value::heap_ptr(...)` instead of
  `Value::from_ptr(1, ...)`.
- `Heap::mark_object` traces cons `car`/`cdr` by decoding the raw u64
  bits as `Value` and following any `TAG_HEAP_PTR` payloads. Before
  this patch the mark phase never walked cons children.
- `Value::is_cons` now returns true for both `TAG_GC_PTR` (legacy
  side-table) and `TAG_HEAP_PTR` (real heap) so `is_list` stays correct
  as call sites migrate.
- `Value::cons_car` / `Value::cons_cdr` (both `unsafe`, both currently
  unused) now operate on `TAG_HEAP_PTR` cells only — they used to
  dereference a side-table index as a raw pointer, which was UB waiting
  to happen.

### Deferred

- Migrating any `LispObject::cons` call site to `Heap::cons_value` —
  Phase 2b, starting with the reader (35 call sites).
- String / Vector / HashTable / BytecodeFn heap allocation — Phase 2c.
- Removing the `HEAP_OBJECTS` side-table — Phase 2d.
- Dropping `Clone` from `LispObject` — final Phase 2 step.

# 0.6.3

## Phase 1b — Environment keyed by SymbolId + function/value cells wired

The Environment now stores `HashMap<SymbolId, LispObject>` instead of
`HashMap<String, LispObject>`; special variables and the specpdl
dynamic-binding stack are keyed by `SymbolId`; `HashKey::Symbol(SymbolId)`
replaces `HashKey::Symbol(String)`. Public Environment API (`get`, `set`,
`define`, `get_function`) still accepts `&str` and interns at the boundary
— no existing call site needed to change.

**Full Lisp-2 flip for global bindings**: `Interpreter::define`,
`defun`, `defalias`, `fset` now write the symbol's function cell
directly (when the value is callable) instead of the global environment.
`defvar` / `defconst` / `set` write the value cell. Global env keeps
only the bootstrap bindings `nil` and `t`.

### Read fallback order

- `Environment::get(name)`: walks the lexical env chain; falls back to
  the symbol's **value cell**. Used by `symbol-value`, `boundp`, varref.
- `Environment::get_function(name)`: walks the lexical env chain for a
  callable binding; falls back to the symbol's **function cell**. Used
  by function-position dispatch in `call_function`, `resolve_function`,
  `symbol-function`, `fboundp`.
- `Environment::get_id_local(id)`: env-only, no cell fallback. Used by
  `defvar`'s already-bound check so process-global value cells don't
  prevent interpreter-local initialisation.

### Behaviour changes

- `boundp` / `symbol-value` return t / the value for any symbol with a
  populated value cell globally — matches Emacs.
- `fboundp` / `symbol-function` report the function cell for any symbol
  with one anywhere in the process. Tests that define functions need
  unique names to avoid cross-talk.
- `set` now writes the value cell directly instead of the environment.
  Lexical shadows in lambdas / let are not touched by `set`, matching
  Emacs semantics more closely.

### Added

- `Environment::get_id`, `set_id`, `define_id`, `get_function_id`,
  `get_id_local` — SymbolId-keyed hot paths.
- `obarray::{get_value_cell, set_value_cell, get_function_cell,
  set_function_cell, get_flags, mark_special}` module-level helpers.
- `obarray::intern` gains a read-lock fast-path (uses read-lock to find
  an existing symbol; only upgrades to write-lock to create a new one).
- Four new Phase 1b tests: `test_defun_writes_function_cell`,
  `test_defvar_writes_value_cell_and_mirrored_in_env`,
  `test_fset_writes_function_cell`, `test_hashkey_symbol_with_eq_test`.

### Deferred

- Function-position dispatch that checks the function cell **first**
  (before walking env chain). Currently lexical bindings shadow the
  function cell, which is correct for let/lambda but slower than a
  direct cell read in the common case.
- Replacing `LispObject::Cons(Arc<Mutex<…>>)` with GC pointers (Phase 2).
- Migrating `autoloads` / `macros` / `features` tables to `SymbolId`.

---

# 0.6.2

## Phase 1a — Symbol cells + plist migration

Property lists now live on `SymbolData` in the global obarray instead of
a separate `InterpreterState.plists` table keyed by `"sym:prop"` string
concatenation. Value-cell and function-cell slots are added to
`SymbolData` for Phase 1b; eval and VM do not yet read them.

### Behaviour change

- Property lists are now **process-global** (one obarray, shared across
  all `Interpreter` instances in a process) rather than
  per-interpreter. This matches Emacs' actual symbol semantics but is a
  change from prior per-instance isolation. Tests that set symbol
  properties should use unique symbol names to avoid cross-talk.
- `(garbage-collect)` now returns `(bytes-allocated . N)` as its first
  pair instead of the misleading `(conses . N)` label — the value was
  always `bytes_allocated()` from the heap, not a cons count.
  `(cons-total . N)` continues to report the true allocation count.

### Added

- `SymbolData` fields: `value: Option<LispObject>`,
  `function: Option<LispObject>`, `plist: Vec<(SymbolId, LispObject)>`.
- `SymbolTable` methods: `get_plist`, `put_plist`, `full_plist`,
  `set_value_cell`, `get_value_cell`, `set_function_cell`,
  `get_function_cell`.
- Module-level `obarray::{get_plist, put_plist, full_plist}` wrappers
  that take `GLOBAL_OBARRAY`'s RwLock.
- Real `symbol-plist` — returns the plist as a `(prop val prop val ...)`
  cons list, preserving insertion order.
- Three new tests in `eval/tests.rs`:
  `test_plist_put_get_roundtrip`,
  `test_plist_put_replaces_in_place`,
  `test_symbol_plist_returns_full_list`.

### Removed

- `InterpreterState.plists` field.
- `PlistTable` type alias (`Arc<RwLock<HashMap<String, LispObject>>>`).

### Deferred to Phase 1b

- `Environment.bindings` / `special_vars` keyed by `SymbolId` instead
  of `String`.
- Value cell / function cell wired into evaluator and VM (currently
  unused storage).
- Fast-path read-lock in `obarray::intern`.
- `HashKey::Symbol(String)` → `SymbolId` migration.

---

# 0.6.1

## Full Emacs Lisp Interpreter with Bytecode VM and Cranelift JIT

Initial release of the `gpui-elisp` crate as a comprehensive Emacs Lisp
interpreter targeting Emacs 30.x standard library compatibility.

### Reader

- Full Emacs Lisp syntax: S-expressions, dotted pairs `(a . b)`,
  backquote/unquote/splice (`` ` ``, `,`, `,@`), `#'` function shorthand
- Character literals: `?a`, `?\n`, `?\x41`, `?\M-\C-@` (meta/control modifiers)
- Number literals: integers, floats (`3.14`, `1e10`, `1.5e-3`), hex (`#xff`),
  octal (`#o77`), binary (`#b1010`), symbols-starting-with-digits (`1value`, `1+`)
- `#[arglist bytecode constants maxdepth]` bytecode function literals
- `.elc` file format: `#@` doc-string skip, `#$` file reference, octal string
  escapes (`\207`), raw control bytes
- `[vector]` literals as self-evaluating `LispObject::Vector`
- `;` line comments, `\`-escaped symbols (`\``, `\,`, `\,@`)
- Keyword symbols (`:test`, `:key`) are self-evaluating

### Evaluator

- ~60 special forms: `quote`, `if` (implicit progn for else), `setq` (multi-pair),
  `defun`, `defvar`/`defconst`, `defalias`/`defsubst`, `defmacro`/`macroexpand`,
  `let`/`let*` (bare symbol bindings), `progn`/`prog1`/`prog2`, `while`/`dolist`,
  `lambda`/`function`, `cond`, `and`/`or`/`when`/`unless`,
  `catch`/`throw`, `condition-case`/`signal`, `unwind-protect`
- Higher-order: `mapcar`, `mapc`, `mapconcat`, `funcall`, `apply` (variadic),
  `eval`, `sort` (with predicate function)
- Module system: `provide`, `featurep`, `require`
- Property lists: `put`, `get` (per-symbol plist table)
- String formatting: `format`/`message` with `%s`, `%d`, `%f`, `%c`, `%x`, `%o`,
  `%S`, `%%`, field width, zero-padding, left-alignment
- Regex: `string-match`/`string-match-p` with Emacs-to-Rust regex translation
- Symbol operations: `symbol-value`, `symbol-function`, `default-value`,
  `default-boundp`, `set-default`, `set`, `boundp`, `fboundp`, `intern`,
  `intern-soft`, `fset`, `make-symbol`
- I/O: `read-from-string`, `split-string`, `version-to-list`, `autoload`
- Hash tables: `make-hash-table` (with `:test`), `gethash`, `puthash`,
  `hash-table-p`, `hash-table-count`, `clrhash`
- Backquote expansion via `backquote.el` macro system (Lisp-2 aware dispatch)
- `error` function with format string substitution
- Stub support for ~40 editor/keymap/buffer functions for stdlib loading

### Primitives (89 builtin functions)

- Arithmetic: `+`, `-`, `*`, `/` (integer-preserving), `1+`, `1-`, `mod`, `abs`,
  `max`, `min`, `floor`, `ceiling`, `round`, `truncate`, `float`, `ash`,
  `logand`, `logior`, `lognot`, `/=`
- Comparison: `=` (exact IEEE 754), `<`, `>`, `<=`, `>=`
- List: `cons`, `car`, `cdr`, `list`, `length`, `nth`, `nthcdr`, `append`,
  `reverse`, `nreverse`, `nconc`, `member`, `memq`, `assoc`, `assq`, `delq`,
  `last`, `copy-sequence`, `make-list`, `cadr`, `cddr`, `caar`, `cdar`,
  `car-safe`, `cdr-safe`, `setcar`, `setcdr` (true in-place mutation)
- Type predicates: `atom`, `symbolp` (includes nil/t), `numberp`, `integerp`,
  `floatp`, `stringp`, `consp`, `listp`, `vectorp`, `functionp`, `subrp`,
  `zerop`, `natnump`, `null`, `not`, `boundp`, `fboundp`
- String: `string=`, `string<`, `concat` (strings only), `substring`,
  `string-to-number`, `number-to-string`, `make-string`, `prin1-to-string`
- Symbol: `symbol-name`, `eq` (atom identity), `equal` (deep equality)
- I/O: `princ`, `prin1`, `identity`, `ignore`, `type-of`

### Error System

- `catch`/`throw` non-local exits with tag matching and propagation
- `condition-case`/`signal` with Emacs error symbols (`void-function`,
  `void-variable`, `wrong-type-argument`, `arith-error`, `invalid-read-syntax`)
- `unwind-protect` with unconditional cleanup
- `error` function with format-string substitution
- `ElispError::Throw`/`Signal` boxed variants for efficient Result types

### Bytecode VM (~80 opcodes)

- Stack: `stack-ref` (0-7), `dup`, `discard`, `discardN`, `stack-set`
- Variables: `varref`, `varset`, `varbind`, `unbind` (with dynamic binding stack)
- Function calls: `call` (0-7 args), delegates to `call_function` for lambdas,
  primitives, bytecode functions, and symbol indirection
- Arithmetic: `add1`, `sub1`, `plus`, `diff`, `mult`, `quo`, `rem`, `negate`
  (all with exact integer semantics and proper type errors)
- Comparison: `eqlsign`, `gtr`, `lss`, `leq`, `geq` (exact, type-checked)
- List: `car`, `cdr`, `cons`, `list1`-`list4`, `length`, `nth`, `nthcdr`,
  `setcar`, `setcdr` (mutation), `car-safe`, `cdr-safe`, `nconc`, `memq`,
  `member`, `assq`, `eq`, `equal`, `not`, `symbolp`, `consp`, `stringp`,
  `listp`, `numberp`, `integerp`
- String: `concat2`-`concat4`, `concatN`, `string=`, `string<`, `substring`
- Array: `aref`, `aset` (mutation via `Arc<Mutex<>>`)
- Control flow: `goto`, `goto-if-nil`, `goto-if-not-nil`, `goto-if-nil-else-pop`,
  `goto-if-not-nil-else-pop`, `return`
- Symbol: `symbol-value`, `symbol-function`, `set`, `fset`, `get` (plist query)
- Variadic: `listN`, `concatN`, `insertN`
- Constants: `constant[0..63]` (opcodes 192-255)

### Interior Mutability

- Cons cells: `Arc<Mutex<(LispObject, LispObject)>>` — `setcar`/`setcdr`
  mutate in place, `nconc` destructively modifies last cdr
- Vectors: `Arc<Mutex<Vec<LispObject>>>` — `aset` modifies elements in place
- Hash tables: `Arc<Mutex<LispHashTable>>` — `puthash` visible through
  original variable binding
- Manual `PartialEq` implementation for deep equality through locks

### Garbage Collector (skeleton)

- `Heap`: mark-and-sweep with adaptive threshold
- `Arena<T>`: bump allocator with free-list recycling for fixed-size objects
- `ConsCell`: `#[repr(C)]` with `GcHeader` prefix and `u64` car/cdr
- `RootGuard`: RAII root stack management
- Stress-tested: 10K allocations, GC collects unreachable, roots survive

### NaN-boxed Value Type

- 64-bit `Copy` type using negative quiet NaN space for tagging
- Immediate types: fixnums (48-bit signed), floats (raw IEEE 754),
  nil/t/unbound, characters, symbol IDs, subr indices
- GC pointers for heap-allocated objects
- `LispObject` <-> `Value` bridge for incremental migration

### Cranelift JIT (feature-gated: `jit`)

- Profiler: invocation counters with configurable compilation threshold
- Compiler: translates 17 bytecode opcodes to Cranelift IR
  - Fast-path fixnum arithmetic with NaN-box tag guards
  - Sign-correct 48-bit payload extraction and re-tagging
  - Pre-scan for jump targets -> Cranelift basic blocks
  - 0-4 arg function pointer trampolines
- Graceful fallback: unsupported opcodes -> VM execution
- Wired into eval dispatch: hot functions auto-compile, deoptimize falls
  through to bytecode VM

### Standard Library Compatibility

- `debug-early.el`: 100% (5/5 forms)
- `byte-run.el`: 100% (~55 forms)
- `backquote.el`: 100% (~15 forms)
- `subr.el`: 99.8% (493/494 forms)
- `subr.elc`: 636 forms parsed
- End-to-end: Emacs 30.2 compiles functions, `.elc` loads, VM executes
  (including recursive factorial)

### Known Limitations

- No real obarray / symbol interning (symbols are strings; deferred to GC
  migration)
- `Rc<RefCell<>>` mutation model, not true GC — cycles will leak
- Lexical binding only (no dynamic binding flag per file)
- `match-data` / regex capture groups not tracked
- Many buffer/editor opcodes are stubs in the VM (work via EditorCallbacks
  in interpreter mode)
- `apply` with >2 args: variadic support in progress
- `split-string` separator argument: in progress
