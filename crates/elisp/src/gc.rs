//! Mark-and-sweep garbage collector with arena allocators.
//!
//! Single-threaded GC designed for the Emacs Lisp interpreter.
//! Uses a mark-and-sweep strategy with:
//! - A typed arena for fixed-size cons cells (24 bytes each)
//! - A general allocator for variable-size objects (strings, vectors, etc.)
//! - An explicit root stack with RAII guards
//! - Allocation-triggered collection when `bytes_allocated > gc_threshold`

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;

// ---------------------------------------------------------------------------
// ObjectTag — discriminant stored in every GC header
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectTag {
    Cons = 0,
    String = 1,
    Vector = 2,
    HashTable = 3,
    ByteCode = 4,
    Symbol = 5,
    Bignum = 6,
    /// Identity-preserving cons cell that wraps the same
    /// `Arc<Mutex<(LispObject, LispObject)>>` as `LispObject::Cons`.
    /// Used by `obj_to_value` migrations where `setcar`/`setcdr`
    /// semantics must survive the Value round-trip. Contrast with
    /// `Cons`, which stores `car`/`cdr` as raw `Value::raw()` bits
    /// for bump-allocation efficiency (used by native Value-based
    /// list builders: `sort`, `nreverse`, `garbage-collect`, etc.).
    ConsArc = 7,
    /// Builtin primitive (subr). Wraps a `String` name; the actual
    /// Rust fn pointer is resolved by `add_primitives`' dispatch
    /// table, keyed by name. Immutable value, no identity concerns.
    Primitive = 8,
}

// ---------------------------------------------------------------------------
// ConsCell — the fundamental Lisp pair
// ---------------------------------------------------------------------------

/// A cons cell (pair) allocated on the GC heap.
///
/// `car` and `cdr` are `u64` placeholders that will become `Value` once the
/// GC is wired into the interpreter. For now they hold arbitrary test data.
#[repr(C)]
pub struct ConsCell {
    pub header: GcHeader,
    pub car: u64,
    pub cdr: u64,
}

// ---------------------------------------------------------------------------
// ConsArcCell — identity-preserving cons cell
// ---------------------------------------------------------------------------

/// A cons cell allocated on the GC heap that wraps the same
/// `Arc<Mutex<(LispObject, LispObject)>>` as `LispObject::Cons`, so
/// `obj_to_value`/`value_to_obj` round-trips preserve pointer
/// identity — `setcar` / `setcdr` mutations propagate, and
/// `(eq x x)` stays true.
///
/// Contrast with `ConsCell`, which stores `car`/`cdr` as raw u64
/// `Value` bits for bump-allocation efficiency and is used by
/// native list builders that don't need `Arc` identity (`sort`,
/// `nreverse`, `garbage-collect`, etc.).
#[repr(C)]
pub struct ConsArcCell {
    pub header: GcHeader,
    pub arc: crate::object::ConsCell,
}

// ---------------------------------------------------------------------------
// StringObject — heap-allocated variable-length string
// ---------------------------------------------------------------------------

/// A string allocated on the GC heap.
///
/// Unlike `ConsCell`, strings are variable-length and don't fit in a typed
/// arena, so they are individually `Box`-allocated. Sweep reconstructs the
/// `Box<StringObject>` via `Box::from_raw` to run the Drop glue on the
/// contained `Box<str>`.
///
/// `#[repr(C)]` pins `header` at offset zero so a `*mut GcHeader` cast from
/// a `*mut StringObject` is always valid.
#[repr(C)]
pub struct StringObject {
    pub header: GcHeader,
    pub data: Box<str>,
}

// ---------------------------------------------------------------------------
// VectorObject — heap-allocated fixed-length array of Values
// ---------------------------------------------------------------------------

/// A Lisp vector allocated on the GC heap.
///
/// Phase 2n: wraps the existing `SharedVec`
/// (`Arc<Mutex<Vec<LispObject>>>`) so `obj_to_value` round-trips on a
/// `LispObject::Vector(arc)` preserve pointer identity — both the
/// heap's Arc and the caller's Arc reference the same inner
/// `Vec<LispObject>`, so `(eq x x)` and `aset`/`aref` mutation
/// propagate correctly. Tracing becomes a no-op for this type:
/// element lifetimes are governed by `Arc` refcounting, not the
/// mark-sweep GC.
#[repr(C)]
pub struct VectorObject {
    pub header: GcHeader,
    pub v: crate::object::SharedVec,
}

// ---------------------------------------------------------------------------
// HashTableObject — heap-allocated hash table
// ---------------------------------------------------------------------------

/// A Lisp hash table allocated on the GC heap.
///
/// Phase 2n: wraps the existing `SharedHashTable`
/// (`Arc<Mutex<LispHashTable>>`) so `obj_to_value` round-trips
/// preserve pointer identity — `puthash`/`gethash` on the same
/// decoded binding see the same underlying map. Tracing is a no-op:
/// keys and values are `LispObject`, not `Value`, and their
/// lifetimes are governed by `Arc` refcounting.
#[repr(C)]
pub struct HashTableObject {
    pub header: GcHeader,
    pub table: crate::object::SharedHashTable,
}

// ---------------------------------------------------------------------------
// BytecodeFnObject — heap-allocated bytecode function
// ---------------------------------------------------------------------------

/// A compiled bytecode function allocated on the GC heap. Wraps the
/// existing `BytecodeFunction` whose internal fields (constants vector
/// of `LispObject`, instruction bytes, stack depth, arity, docstring)
/// are unchanged. No child tracing yet — constants are `LispObject`.
#[repr(C)]
pub struct BytecodeFnObject {
    pub header: GcHeader,
    pub func: crate::object::BytecodeFunction,
}

// ---------------------------------------------------------------------------
// BignumObject — heap-allocated big integer
// ---------------------------------------------------------------------------

/// A big integer that doesn't fit in a 48-bit fixnum Value. For now
/// this just stores an `i64` — oversized integers in the current
/// codebase never exceed `i64::MAX`, and wrapping Emacs's arbitrary-
/// precision semantics is a separate project. The object is here so
/// `Heap` can replace the side-table fallback path.
#[repr(C)]
pub struct BignumObject {
    pub header: GcHeader,
    pub value: i64,
}

// ---------------------------------------------------------------------------
// PrimitiveObject — heap-allocated builtin function reference
// ---------------------------------------------------------------------------

/// A builtin primitive (subr) allocated on the GC heap. Wraps the
/// primitive's name as an owned `String`; dispatch is handled at
/// call time by looking up the name in the primitives table. This
/// mirrors the `LispObject::Primitive(String)` representation and
/// unblocks Phase 2o's removal of the `HEAP_OBJECTS` side-table.
#[repr(C)]
pub struct PrimitiveObject {
    pub header: GcHeader,
    pub name: String,
}

// ---------------------------------------------------------------------------
// GcHeader — prefix for every GC-managed object
// ---------------------------------------------------------------------------

/// Every heap-allocated Lisp object is prefixed with this header.
/// The intrusive linked list (`next`) threads all live objects for sweeping.
#[repr(C)]
pub struct GcHeader {
    pub tag: ObjectTag,
    pub marked: bool,
    pub next: *mut GcHeader,
}

impl GcHeader {
    pub fn new(tag: ObjectTag) -> Self {
        Self {
            tag,
            marked: false,
            next: ptr::null_mut(),
        }
    }
}

// ---------------------------------------------------------------------------
// GcMode — controls when automatic collection fires
// ---------------------------------------------------------------------------

/// Controls whether allocations may trigger a collection.
///
/// `Auto` preserves the original behaviour: every allocation past the
/// threshold triggers a sweep via `maybe_gc`. `Manual` disables that
/// implicit trigger so only explicit `Heap::collect()` calls sweep.
///
/// The interpreter runs in `Manual` mode (see `Interpreter::new`) so that
/// GC only happens at well-defined safepoints — the `(garbage-collect)`
/// primitive — and never interrupts a multi-step allocation sequence
/// holding unrooted intermediate Values on the stack. Standalone `Heap`
/// instances (e.g. in the gc.rs unit tests) default to `Auto`, which
/// keeps the existing threshold-driven tests honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcMode {
    Auto,
    Manual,
}

// ---------------------------------------------------------------------------
// Heap — the main GC state
// ---------------------------------------------------------------------------

/// Central garbage-collected heap.
///
/// `Heap` is intentionally `!Send` and `!Sync` — the GC is single-threaded.
pub struct Heap {
    /// Head of the intrusive linked list of all allocated objects.
    all_objects: *mut GcHeader,
    /// Running total of bytes allocated through this heap.
    bytes_allocated: usize,
    /// Collection is triggered when `bytes_allocated > gc_threshold`.
    gc_threshold: usize,
    /// Number of collections performed so far.
    gc_count: u64,
    /// Controls whether `maybe_gc` fires automatically past the threshold.
    gc_mode: GcMode,
    /// Explicit root stack. Entries are raw pointers to GcHeaders that the
    /// mutator considers live. Managed via `RootGuard` RAII handles.
    root_stack: Vec<*const GcHeader>,
    /// Typed arena for fixed-size cons cells (u64 car/cdr).
    cons_arena: Arena<ConsCell>,
    /// Typed arena for identity-preserving `ConsArcCell`s. Sweep
    /// `drop_in_place`s the contained `Arc` before returning the
    /// slot to the arena's free list; the next allocation
    /// overwrites the slot via `ptr::write` without reading its
    /// (dropped) contents first.
    cons_arc_arena: Arena<ConsArcCell>,
    /// Prevent Send/Sync.
    _not_send: PhantomData<*mut ()>,
}

/// Default initial GC threshold (256 KiB).
const DEFAULT_GC_THRESHOLD: usize = 256 * 1024;

impl Heap {
    /// Create a new, empty heap with the default GC threshold.
    /// Default arena page size: 1024 cons cells per page.
    const CONS_PAGE_SIZE: usize = 1024;

    pub fn new() -> Self {
        Self {
            all_objects: ptr::null_mut(),
            bytes_allocated: 0,
            gc_threshold: DEFAULT_GC_THRESHOLD,
            gc_count: 0,
            gc_mode: GcMode::Auto,
            root_stack: Vec::new(),
            cons_arena: Arena::new(Self::CONS_PAGE_SIZE),
            // Phase 4b: ConsArcCells also get a typed arena. Sized a
            // bit smaller than the cons_arena since they're less
            // frequent (only created by `obj_to_value(LispObject::Cons)`
            // migrations, not native list-building).
            cons_arc_arena: Arena::new(Self::CONS_PAGE_SIZE),
            _not_send: PhantomData,
        }
    }

    /// Set the GC threshold (useful for testing).
    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.gc_threshold = threshold;
    }

    /// Switch between automatic and manual GC policy. See [`GcMode`].
    pub fn set_gc_mode(&mut self, mode: GcMode) {
        self.gc_mode = mode;
    }

    /// Current collection policy.
    pub fn gc_mode(&self) -> GcMode {
        self.gc_mode
    }

    /// Returns `true` when the heap has exceeded its allocation threshold
    /// and a collection should be triggered.
    pub fn should_gc(&self) -> bool {
        self.bytes_allocated > self.gc_threshold
    }

    // -- Cons allocation ----------------------------------------------------

    /// Allocate a cons cell on the heap. Triggers GC if the threshold is
    /// exceeded.
    ///
    /// Returns a raw pointer to the newly allocated `ConsCell`. The pointer
    /// is valid until the next GC cycle, unless the cell is reachable from
    /// a root.
    pub fn cons(&mut self, car: u64, cdr: u64) -> *mut ConsCell {
        self.maybe_gc();

        let cell = self.cons_arena.alloc();
        // SAFETY: `cell` points to uninitialised arena memory that we now
        // fully initialise before anyone reads it.
        unsafe {
            ptr::write(
                cell,
                ConsCell {
                    header: GcHeader::new(ObjectTag::Cons),
                    car,
                    cdr,
                },
            );
            // Link into the all_objects intrusive list.
            (*cell).header.next = self.all_objects;
            self.all_objects = &mut (*cell).header;
        }
        self.bytes_allocated += std::mem::size_of::<ConsCell>();
        cell
    }

    /// Allocate a cons cell with Value car/cdr and return a Value
    /// tagged as a real heap pointer (TAG_HEAP_PTR).
    ///
    /// This is the preferred allocation path for code that works with
    /// `Value` directly, avoiding the LispObject round-trip. The resulting
    /// Value is traceable by mark-and-sweep via `Value::trace`.
    ///
    /// Note: this produces a `ConsCell` (u64 car/cdr, bump-allocated)
    /// not a `ConsArcCell`. `value_to_obj` of the resulting Value
    /// decodes into a FRESH `LispObject::Cons(Arc::new(...))` — Arc
    /// identity is not preserved across round-trips. For callers that
    /// need identity-preserving semantics (obj_to_value migrations),
    /// use `cons_arc_value` instead.
    pub fn cons_value(&mut self, car: u64, cdr: u64) -> crate::value::Value {
        let cell = self.cons(car, cdr);
        crate::value::Value::heap_ptr(cell as *const u8)
    }

    // -- Identity-preserving cons allocation --------------------------------

    /// Allocate a `ConsArcCell` on the heap wrapping an existing
    /// `Arc<Mutex<(LispObject, LispObject)>>`. Returns a Value tagged
    /// as a real heap pointer with `ObjectTag::ConsArc`.
    ///
    /// Phase 2n-cons chokepoint for migrating `LispObject::Cons(arc)`
    /// through `obj_to_value` while preserving `setcar`/`setcdr`
    /// identity.
    ///
    /// Phase 4b: the cell is bump-allocated from a typed `Arena`
    /// (matching the `ConsCell` path) rather than individually
    /// `Box`-allocated. Sweep `drop_in_place`s the Arc before
    /// returning the slot to the free list.
    pub fn cons_arc_value(&mut self, arc: crate::object::ConsCell) -> crate::value::Value {
        self.maybe_gc();
        let ptr = self.cons_arc_arena.alloc();
        // SAFETY: `ptr` points at uninitialised memory from the arena.
        // `ptr::write` initialises the slot before any read happens.
        unsafe {
            std::ptr::write(
                ptr,
                ConsArcCell {
                    header: GcHeader::new(ObjectTag::ConsArc),
                    arc,
                },
            );
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += std::mem::size_of::<ConsArcCell>();
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    // -- String allocation --------------------------------------------------

    /// Allocate a `StringObject` on the heap, copying `s` into owned
    /// `Box<str>` storage. Returns a Value tagged as a real heap
    /// pointer (TAG_HEAP_PTR); callers disambiguate strings from cons
    /// cells by inspecting the pointed-to `GcHeader.tag`.
    ///
    /// The returned Value is traced by `mark_object` (no children —
    /// strings are leaves) and freed by `sweep` via `Box::from_raw`
    /// when unreachable.
    pub fn string_value(&mut self, s: &str) -> crate::value::Value {
        self.maybe_gc();
        let boxed = Box::new(StringObject {
            header: GcHeader::new(ObjectTag::String),
            data: Box::<str>::from(s),
        });
        let size = std::mem::size_of::<StringObject>() + s.len();
        let ptr: *mut StringObject = Box::into_raw(boxed);
        // SAFETY: `ptr` was just produced by `Box::into_raw` and points
        // to a fully-initialised `StringObject`. Linking it into the
        // intrusive all_objects list is the standard registration step
        // the sweep phase relies on.
        unsafe {
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += size;
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    // -- Vector allocation --------------------------------------------------

    /// Allocate a `VectorObject` on the heap wrapping an existing
    /// `SharedVec`. Returns a Value tagged as a real heap pointer.
    ///
    /// Phase 2n: the caller owns the Arc; the heap stores a clone.
    /// `value_to_obj` returns another `Arc::clone` — identity is
    /// preserved across `obj_to_value`/`value_to_obj` round-trips.
    pub fn vector_value(&mut self, v: crate::object::SharedVec) -> crate::value::Value {
        self.maybe_gc();
        let size = std::mem::size_of::<VectorObject>();
        let boxed = Box::new(VectorObject {
            header: GcHeader::new(ObjectTag::Vector),
            v,
        });
        let ptr: *mut VectorObject = Box::into_raw(boxed);
        // SAFETY: fresh Box::into_raw produces a valid pointer.
        unsafe {
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += size;
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    // -- HashTable allocation -----------------------------------------------

    /// Allocate a `HashTableObject` on the heap wrapping an existing
    /// `SharedHashTable`. Returns a Value tagged as a real heap pointer.
    ///
    /// Phase 2n: identity-preserving, as for vectors.
    pub fn hashtable_value(
        &mut self,
        table: crate::object::SharedHashTable,
    ) -> crate::value::Value {
        self.maybe_gc();
        let boxed = Box::new(HashTableObject {
            header: GcHeader::new(ObjectTag::HashTable),
            table,
        });
        let ptr: *mut HashTableObject = Box::into_raw(boxed);
        // SAFETY: fresh Box::into_raw produces a valid pointer.
        unsafe {
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += std::mem::size_of::<HashTableObject>();
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    // -- Bytecode function allocation ---------------------------------------

    /// Allocate a `BytecodeFnObject` wrapping an existing
    /// `BytecodeFunction`. Returns a Value tagged as a real heap pointer.
    pub fn bytecode_value(&mut self, func: crate::object::BytecodeFunction) -> crate::value::Value {
        self.maybe_gc();
        let boxed = Box::new(BytecodeFnObject {
            header: GcHeader::new(ObjectTag::ByteCode),
            func,
        });
        let ptr: *mut BytecodeFnObject = Box::into_raw(boxed);
        // SAFETY: fresh Box::into_raw produces a valid pointer.
        unsafe {
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += std::mem::size_of::<BytecodeFnObject>();
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    // -- Primitive allocation -----------------------------------------------

    /// Allocate a `PrimitiveObject` wrapping a primitive's name.
    /// Returns a Value tagged as a real heap pointer (TAG_HEAP_PTR
    /// with `ObjectTag::Primitive`).
    ///
    /// Phase 2o chokepoint: `obj_to_value(LispObject::Primitive(name))`
    /// under a HeapScope allocates one of these instead of pushing
    /// into the thread-local side-table.
    pub fn primitive_value(&mut self, name: &str) -> crate::value::Value {
        self.maybe_gc();
        let size = std::mem::size_of::<PrimitiveObject>() + name.len();
        let boxed = Box::new(PrimitiveObject {
            header: GcHeader::new(ObjectTag::Primitive),
            name: name.to_string(),
        });
        let ptr: *mut PrimitiveObject = Box::into_raw(boxed);
        // SAFETY: fresh Box::into_raw produces a valid pointer.
        unsafe {
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += size;
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    // -- Bignum allocation --------------------------------------------------

    /// Allocate a `BignumObject` holding an integer outside the 48-bit
    /// fixnum range. Returns a Value tagged as a real heap pointer.
    pub fn bignum_value(&mut self, n: i64) -> crate::value::Value {
        self.maybe_gc();
        let boxed = Box::new(BignumObject {
            header: GcHeader::new(ObjectTag::Bignum),
            value: n,
        });
        let ptr: *mut BignumObject = Box::into_raw(boxed);
        // SAFETY: fresh Box::into_raw produces a valid pointer.
        unsafe {
            (*ptr).header.next = self.all_objects;
            self.all_objects = &mut (*ptr).header;
        }
        self.bytes_allocated += std::mem::size_of::<BignumObject>();
        crate::value::Value::heap_ptr(ptr as *const u8)
    }

    /// Trigger a GC cycle if the allocation threshold has been exceeded
    /// and the heap is in `Auto` mode. In `Manual` mode this is a no-op —
    /// only explicit `Heap::collect()` calls sweep.
    fn maybe_gc(&mut self) {
        if self.gc_mode == GcMode::Manual {
            return;
        }
        if self.should_gc() {
            self.collect();
        }
    }

    /// Run a full mark-and-sweep collection.
    pub fn collect(&mut self) {
        self.mark_roots();
        self.sweep();
        self.gc_count += 1;

        // Adaptive threshold: grow to 2x live size, but never below the default.
        self.gc_threshold = (self.bytes_allocated * 2).max(DEFAULT_GC_THRESHOLD);
    }

    /// Total bytes currently attributed to this heap.
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
    }

    /// Number of collections performed.
    pub fn gc_count(&self) -> u64 {
        self.gc_count
    }

    // -- Root management ----------------------------------------------------

    /// Push a root onto the root stack. Returns an index that `pop_root` uses.
    ///
    /// Callers should prefer the RAII `RootGuard` instead of calling this
    /// directly.
    pub fn push_root(&mut self, root: *const GcHeader) -> usize {
        let idx = self.root_stack.len();
        self.root_stack.push(root);
        idx
    }

    /// Pop the root at `idx`. The index **must** be the value returned by
    /// `push_root`, and roots must be popped in reverse order (LIFO).
    pub fn pop_root(&mut self, idx: usize) {
        debug_assert_eq!(
            idx,
            self.root_stack.len() - 1,
            "roots must be popped in LIFO order"
        );
        self.root_stack.pop();
    }

    // -- Registration -------------------------------------------------------

    /// Register an externally-allocated object with the heap so it
    /// participates in the sweep phase. Prepends to the all_objects list.
    ///
    /// # Safety
    /// `header` must point to a valid `GcHeader` that will remain valid until
    /// it is freed by `sweep` or the heap is dropped.
    pub unsafe fn register(&mut self, header: *mut GcHeader, size: usize) {
        // SAFETY: caller guarantees header is valid.
        unsafe {
            (*header).next = self.all_objects;
        }
        self.all_objects = header;
        self.bytes_allocated += size;
    }

    // -- Mark phase ---------------------------------------------------------

    fn mark_roots(&mut self) {
        for i in 0..self.root_stack.len() {
            let root = self.root_stack[i];
            if !root.is_null() {
                // SAFETY: roots are guaranteed valid by the push_root contract.
                unsafe {
                    Self::mark_object(root as *mut GcHeader);
                }
            }
        }
    }

    /// Mark an object and all objects reachable from it.
    ///
    /// Uses an explicit work-stack instead of recursion to avoid blowing
    /// the call stack on deeply nested structures.
    ///
    /// # Safety
    /// `header` must point to a valid, heap-allocated `GcHeader`.
    unsafe fn mark_object(header: *mut GcHeader) {
        let mut work: Vec<*mut GcHeader> = Vec::new();
        if !header.is_null() {
            work.push(header);
        }
        while let Some(h) = work.pop() {
            if h.is_null() {
                continue;
            }
            // SAFETY: all pointers pushed onto `work` originate from either
            // the root stack or from the car/cdr of a previously validated
            // cons cell, so they point to valid GcHeaders.
            let hdr = unsafe { &mut *h };
            if hdr.marked {
                continue; // already visited — break cycles
            }
            hdr.marked = true;

            // Trace children. Cons cells store `car` and `cdr` as raw u64
            // bits that are interpreted as `Value`. Only TAG_HEAP_PTR
            // bit-patterns are real heap pointers that need traversal; all
            // other bit-patterns (immediates, side-table indices, or raw
            // test u64s) visit nothing thanks to `Value::trace`.
            let tag = hdr.tag;
            match tag {
                ObjectTag::Cons => {
                    let cell = h as *const ConsCell;
                    // SAFETY: `h` was validated as a live GcHeader above,
                    // and a ConsCell has a GcHeader as its first field, so
                    // the cast and field accesses are sound.
                    let (car_raw, cdr_raw) = unsafe { ((*cell).car, (*cell).cdr) };
                    crate::value::Value::from_raw(car_raw).trace(|p| work.push(p));
                    crate::value::Value::from_raw(cdr_raw).trace(|p| work.push(p));
                }
                ObjectTag::String => {
                    // Strings are leaves — their `data: Box<str>` payload
                    // contains no Lisp Values. Marking is enough.
                }
                ObjectTag::Vector
                | ObjectTag::HashTable
                | ObjectTag::ByteCode
                | ObjectTag::ConsArc
                | ObjectTag::Primitive => {
                    // Phase 2n/2n-cons/2o: these wrap existing
                    // `Arc<Mutex<_>>` containers, owned LispObject
                    // content, or leaf data (Primitive holds a
                    // String). No child Values to trace — lifetimes
                    // are governed by `Arc` refcounting (for shared
                    // containers) or owned data (leaf types).
                }
                ObjectTag::Bignum => {
                    // Leaf — just an i64.
                }
                ObjectTag::Symbol => {
                    // Symbols aren't heap-allocated via `Heap` yet; they
                    // live in the process-global obarray.
                }
            }
        }
    }

    // -- Sweep phase --------------------------------------------------------

    /// Walk the all_objects list and free every unmarked object.
    /// Marked objects have their flag cleared for the next cycle.
    fn sweep(&mut self) {
        let mut prev: *mut *mut GcHeader = &mut self.all_objects;

        // SAFETY: we only dereference pointers that were registered via
        // `register` or `cons`, which guarantee them to be valid.
        unsafe {
            let mut current = *prev;
            while !current.is_null() {
                let header = &mut *current;
                if header.marked {
                    // Object is live — clear mark for next cycle and advance.
                    header.marked = false;
                    prev = &mut header.next;
                    current = header.next;
                } else {
                    // Object is garbage — unlink and deallocate.
                    let next = header.next;
                    *prev = next;

                    let obj_size = Self::object_size(header);
                    self.bytes_allocated = self.bytes_allocated.saturating_sub(obj_size);

                    // Return the slot to the appropriate arena's free list,
                    // or drop the individually-allocated Box for types that
                    // aren't arena-managed.
                    match header.tag {
                        ObjectTag::Cons => {
                            // SAFETY: the header is the first field of a
                            // ConsCell allocated from `cons_arena`, so this
                            // cast is valid.
                            let cons = current as *mut ConsCell;
                            self.cons_arena.free(cons);
                        }
                        ObjectTag::String => {
                            // SAFETY: Box::into_raw-allocated in
                            // `Heap::string_value`; reconstituting + drop
                            // runs the `Box<str>` destructor.
                            let string_ptr = current as *mut StringObject;
                            let _ = Box::from_raw(string_ptr);
                        }
                        ObjectTag::Vector => {
                            // SAFETY: Box::into_raw-allocated in
                            // `Heap::vector_value`.
                            let vec_ptr = current as *mut VectorObject;
                            let _ = Box::from_raw(vec_ptr);
                        }
                        ObjectTag::HashTable => {
                            // SAFETY: Box::into_raw-allocated in
                            // `Heap::hashtable_value`.
                            let ht_ptr = current as *mut HashTableObject;
                            let _ = Box::from_raw(ht_ptr);
                        }
                        ObjectTag::ByteCode => {
                            // SAFETY: Box::into_raw-allocated in
                            // `Heap::bytecode_value`.
                            let bc_ptr = current as *mut BytecodeFnObject;
                            let _ = Box::from_raw(bc_ptr);
                        }
                        ObjectTag::Bignum => {
                            // SAFETY: Box::into_raw-allocated in
                            // `Heap::bignum_value`.
                            let bn_ptr = current as *mut BignumObject;
                            let _ = Box::from_raw(bn_ptr);
                        }
                        ObjectTag::Symbol => {
                            // Symbols aren't allocated through `Heap`
                            // — they live in the process-global obarray.
                            // If a Symbol-tagged header ever reaches
                            // sweep it was externally registered by a
                            // test via `register`, so we just unlink.
                        }
                        ObjectTag::ConsArc => {
                            // SAFETY: arena-allocated in
                            // `Heap::cons_arc_value`. Explicitly
                            // `drop_in_place` runs the `Arc`'s
                            // destructor, decrementing the refcount
                            // (and freeing the inner data if we held
                            // the last reference). The arena then
                            // recycles the slot; its next allocation
                            // overwrites with `ptr::write`, so the
                            // dropped state is never read.
                            let arc_ptr = current as *mut ConsArcCell;
                            std::ptr::drop_in_place(arc_ptr);
                            self.cons_arc_arena.free(arc_ptr);
                        }
                        ObjectTag::Primitive => {
                            // SAFETY: Box::into_raw-allocated in
                            // `Heap::primitive_value`. Dropping the
                            // Box runs the inner `String` destructor.
                            let p_ptr = current as *mut PrimitiveObject;
                            let _ = Box::from_raw(p_ptr);
                        }
                    }

                    current = next;
                }
            }
        }
    }

    /// Return the size in bytes attributed to a GC object based on its tag.
    ///
    /// SAFETY for each arm: when `header.tag == T`, the enclosing object
    /// is the struct corresponding to that tag (enforced by `#[repr(C)]`
    /// with `header` at offset zero). Taking an explicit `&T` first
    /// before calling methods avoids implicit autoref through the raw
    /// pointer, which the `dangerous_implicit_autorefs` lint flags.
    fn object_size(header: &GcHeader) -> usize {
        match header.tag {
            ObjectTag::Cons => std::mem::size_of::<ConsCell>(),
            ObjectTag::String => {
                let string_ptr = header as *const GcHeader as *const StringObject;
                let obj: &StringObject = unsafe { &*string_ptr };
                std::mem::size_of::<StringObject>() + obj.data.len()
            }
            ObjectTag::Vector => {
                // Phase 2n: the inner Vec lives inside the Arc<Mutex<_>>
                // (allocator-managed, shared with LispObject references).
                // Only the wrapper is attributed to the GC heap; inner
                // element memory is governed by Arc refcounting.
                std::mem::size_of::<VectorObject>()
            }
            ObjectTag::HashTable => std::mem::size_of::<HashTableObject>(),
            ObjectTag::ByteCode => std::mem::size_of::<BytecodeFnObject>(),
            ObjectTag::Bignum => std::mem::size_of::<BignumObject>(),
            ObjectTag::Symbol => 0,
            ObjectTag::ConsArc => std::mem::size_of::<ConsArcCell>(),
            ObjectTag::Primitive => {
                let p_ptr = header as *const GcHeader as *const PrimitiveObject;
                let obj: &PrimitiveObject = unsafe { &*p_ptr };
                std::mem::size_of::<PrimitiveObject>() + obj.name.len()
            }
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: Heap is single-threaded by design (uses raw pointers internally),
// but it is safe to Send across threads when protected by a Mutex. All access
// through InterpreterState goes through Arc<Mutex<Heap>>.
unsafe impl Send for Heap {}

// ---------------------------------------------------------------------------
// Arena<T> — bump allocator with a free list
// ---------------------------------------------------------------------------

/// A simple arena that allocates fixed-size `T` objects via bump allocation
/// within pages, with a free list for recycling.
pub struct Arena<T> {
    /// Backing pages. Each page holds `page_size` slots.
    pages: Vec<Box<[MaybeUninit<T>]>>,
    /// Recycled slots available for reuse.
    free_list: Vec<*mut T>,
    /// Next unused slot index within the current (last) page.
    bump_idx: usize,
    /// Number of `T` slots per page.
    page_size: usize,
}

impl<T> Arena<T> {
    /// Create a new arena. `page_size` is the number of `T`-sized slots
    /// per backing page.
    pub fn new(page_size: usize) -> Self {
        assert!(page_size > 0, "page_size must be > 0");
        Self {
            pages: Vec::new(),
            free_list: Vec::new(),
            bump_idx: 0,
            page_size,
        }
    }

    /// Allocate a slot and return a raw pointer to uninitialised memory.
    ///
    /// The caller must write a valid `T` into the returned pointer before
    /// reading from it.
    pub fn alloc(&mut self) -> *mut T {
        // Prefer recycled slots.
        if let Some(ptr) = self.free_list.pop() {
            return ptr;
        }

        // Need a fresh slot — allocate a new page if the current one is full
        // (or if there are no pages yet).
        if self.pages.is_empty() || self.bump_idx >= self.page_size {
            self.add_page();
        }

        let page = self.pages.last_mut().expect("just added a page");
        let ptr = page[self.bump_idx].as_mut_ptr();
        self.bump_idx += 1;
        ptr
    }

    /// Return a previously-allocated slot to the free list.
    ///
    /// # Safety
    /// `ptr` must have been returned by a prior call to `alloc` on this arena,
    /// and must not be used after this call.
    pub unsafe fn free(&mut self, ptr: *mut T) {
        // SAFETY: the caller guarantees `ptr` came from this arena.
        // We do not drop the value — the caller is responsible for dropping
        // before calling free (if T has drop glue).
        self.free_list.push(ptr);
    }

    /// Number of pages currently allocated.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn add_page(&mut self) {
        let page: Vec<MaybeUninit<T>> =
            (0..self.page_size).map(|_| MaybeUninit::uninit()).collect();
        self.pages.push(page.into_boxed_slice());
        self.bump_idx = 0;
    }
}

// ---------------------------------------------------------------------------
// RootGuard — RAII handle for pushing/popping GC roots
// ---------------------------------------------------------------------------

/// RAII guard that pushes a root on creation and pops it on drop.
///
/// This ensures roots are always popped in the correct LIFO order, even in
/// the presence of early returns or panics.
pub struct RootGuard<'heap> {
    heap: &'heap mut Heap,
    idx: usize,
}

impl<'heap> RootGuard<'heap> {
    /// Push `root` onto the heap's root stack and return a guard that will
    /// pop it on drop.
    pub fn new(heap: &'heap mut Heap, root: *const GcHeader) -> Self {
        let idx = heap.push_root(root);
        Self { heap, idx }
    }
}

impl Drop for RootGuard<'_> {
    fn drop(&mut self) {
        self.heap.pop_root(self.idx);
    }
}

// ---------------------------------------------------------------------------
// Value-aware rooting helpers
// ---------------------------------------------------------------------------

impl Heap {
    /// Push `val` onto the root stack if it carries a real heap pointer.
    ///
    /// Returns `Some(index)` that must later be passed to `pop_root`, or
    /// `None` for immediates, side-table indices, and other Values that
    /// don't need rooting. The `None` case is typically safe to ignore.
    ///
    /// The index-returning API mirrors `push_root` / `pop_root` and is
    /// compatible with the existing rooting contract: roots must be
    /// popped in LIFO order. A stricter RAII wrapper would require an
    /// exclusive borrow of `Heap` for the whole scope, which would
    /// prevent the caller from allocating or collecting while holding
    /// the guard — too restrictive for interpreter use.
    pub fn root_value(&mut self, val: crate::value::Value) -> Option<usize> {
        let ptr = val.as_heap_ptr()?;
        Some(self.push_root(ptr as *const GcHeader))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_new_starts_empty() {
        let heap = Heap::new();
        assert_eq!(heap.bytes_allocated(), 0);
        assert_eq!(heap.gc_count(), 0);
        assert!(!heap.should_gc());
    }

    #[test]
    fn heap_collect_on_empty_does_not_crash() {
        let mut heap = Heap::new();
        heap.collect();
        assert_eq!(heap.gc_count(), 1);
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn heap_should_gc_respects_threshold() {
        let mut heap = Heap::new();
        // Manually bump allocated bytes past the threshold.
        heap.bytes_allocated = DEFAULT_GC_THRESHOLD + 1;
        assert!(heap.should_gc());
    }

    #[test]
    fn arena_alloc_returns_distinct_pointers() {
        let mut arena: Arena<u64> = Arena::new(16);
        let a = arena.alloc();
        let b = arena.alloc();
        assert_ne!(a, b);
    }

    #[test]
    fn arena_free_reuses_slot() {
        let mut arena: Arena<u64> = Arena::new(16);
        let a = arena.alloc();

        // SAFETY: `a` was just allocated from this arena and we won't use it
        // after freeing.
        unsafe { arena.free(a) };

        let b = arena.alloc();
        // The free-list should hand back the same pointer.
        assert_eq!(a, b);
    }

    #[test]
    fn arena_grows_pages_when_full() {
        let mut arena: Arena<u8> = Arena::new(4);
        assert_eq!(arena.page_count(), 0);

        // Fill first page.
        for _ in 0..4 {
            arena.alloc();
        }
        assert_eq!(arena.page_count(), 1);

        // One more triggers a second page.
        arena.alloc();
        assert_eq!(arena.page_count(), 2);
    }

    #[test]
    fn heap_register_and_sweep_unmarks_live_objects() {
        let mut heap = Heap::new();
        let mut header = GcHeader::new(ObjectTag::Cons);

        // SAFETY: header is a valid local GcHeader; we keep it alive for the
        // duration of the test.
        unsafe {
            heap.register(&mut header as *mut GcHeader, 24);
        }
        assert_eq!(heap.bytes_allocated(), 24);

        // Push as root so it survives collection.
        heap.push_root(&header as *const GcHeader);
        heap.collect();

        // Object survived, mark cleared for next cycle.
        assert!(!header.marked);
        assert_eq!(heap.gc_count(), 1);
    }

    #[test]
    fn root_push_pop_lifo() {
        let mut heap = Heap::new();
        let h1 = GcHeader::new(ObjectTag::Symbol);
        let h2 = GcHeader::new(ObjectTag::String);

        let idx1 = heap.push_root(&h1 as *const GcHeader);
        let idx2 = heap.push_root(&h2 as *const GcHeader);

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);

        heap.pop_root(idx2);
        heap.pop_root(idx1);
        assert!(heap.root_stack.is_empty());
    }

    // -- Cons allocation tests ----------------------------------------------

    #[test]
    fn cons_basic_allocation() {
        let mut heap = Heap::new();
        let cell = heap.cons(1, 2);
        // SAFETY: cell was just allocated and is valid.
        unsafe {
            assert_eq!((*cell).car, 1);
            assert_eq!((*cell).cdr, 2);
            assert_eq!((*cell).header.tag, ObjectTag::Cons);
            assert!(!(*cell).header.marked);
        }
        assert_eq!(heap.bytes_allocated(), std::mem::size_of::<ConsCell>(),);
    }

    #[test]
    fn cons_cells_linked_into_all_objects() {
        let mut heap = Heap::new();
        let _c1 = heap.cons(1, 0);
        let c2 = heap.cons(2, 0);
        // The most recently allocated cell should be the head of the list.
        assert_eq!(heap.all_objects, unsafe { &mut (*c2).header } as *mut _);
    }

    // -- GC collection tests ------------------------------------------------

    #[test]
    fn gc_collects_unreachable_cons_cells() {
        let mut heap = Heap::new();
        heap.set_gc_threshold(0); // Force GC on every allocation.

        // Allocate a cell with no root — it should be collected.
        let _cell = heap.cons(99, 0);
        // The first cons() doesn't trigger GC (maybe_gc runs before alloc,
        // and threshold is 0 so bytes_allocated=0 is not > 0).
        // Allocate a second cell: now maybe_gc sees bytes > threshold.
        let _cell2 = heap.cons(100, 0);
        assert!(heap.gc_count() > 0, "GC should have triggered");
        // After GC, the first cell (unreachable) should be swept. Only the
        // second cell survives (it was allocated after the sweep).
        assert_eq!(
            heap.bytes_allocated(),
            std::mem::size_of::<ConsCell>(),
            "Only the post-GC cell should remain",
        );
    }

    #[test]
    fn gc_preserves_rooted_cons_cells() {
        let mut heap = Heap::new();
        heap.set_gc_threshold(256);

        let cell = heap.cons(42, 0);
        // SAFETY: cell points to a valid ConsCell whose header is a GcHeader.
        let root_idx = heap.push_root(unsafe { &(*cell).header } as *const GcHeader);

        // Allocate many more cells to trigger GC.
        for i in 0..1000u64 {
            let _ = heap.cons(i, 0);
        }

        assert!(heap.gc_count() > 0, "GC should have triggered");
        // SAFETY: the rooted cell must still be valid.
        unsafe {
            assert_eq!((*cell).car, 42, "Rooted cons cell must survive GC");
        }

        heap.pop_root(root_idx);
    }

    #[test]
    fn gc_stress_cons_cells() {
        let mut heap = Heap::new();
        heap.set_gc_threshold(1024);

        // Allocate 10,000 cons cells with no roots — all should be collected.
        for i in 0..10_000u64 {
            let _ = heap.cons(i, i + 1);
        }

        assert!(heap.gc_count() > 0, "GC should have triggered");
        // bytes_allocated should be bounded — not 10000 * sizeof(ConsCell).
        let uncollected = 10_000 * std::mem::size_of::<ConsCell>();
        assert!(
            heap.bytes_allocated() < uncollected,
            "GC should have collected dead cells: {} bytes remain, \
             {} would be uncollected",
            heap.bytes_allocated(),
            uncollected,
        );
        eprintln!(
            "Stress test: {} bytes after 10000 allocs, {} GC cycles",
            heap.bytes_allocated(),
            heap.gc_count(),
        );
    }

    #[test]
    fn gc_stress_with_roots() {
        let mut heap = Heap::new();
        heap.set_gc_threshold(512);

        // Keep every 100th cell rooted.
        let mut rooted: Vec<(*mut ConsCell, usize)> = Vec::new();
        for i in 0..5_000u64 {
            let cell = heap.cons(i, 0);
            if i % 100 == 0 {
                // SAFETY: cell was just allocated and is valid.
                let idx = heap.push_root(unsafe { &(*cell).header } as *const GcHeader);
                rooted.push((cell, idx));
            }
        }

        assert!(heap.gc_count() > 0);

        // Verify all rooted cells survived with correct data.
        for &(cell, _) in &rooted {
            // SAFETY: rooted cells are kept alive by the root stack.
            unsafe {
                let val = (*cell).car;
                assert_eq!(
                    val % 100,
                    0,
                    "Rooted cell should have a value divisible by 100, got {val}"
                );
            }
        }

        // Pop roots in reverse LIFO order.
        for &(_, idx) in rooted.iter().rev() {
            heap.pop_root(idx);
        }
    }

    // -- Phase 2a: traceable heap cons cells -------------------------------

    #[test]
    fn cons_chain_rooted_survives_gc() {
        use crate::value::Value;
        let mut heap = Heap::new();
        // Keep the default (generous) threshold during construction so no GC
        // fires mid-build — the intermediate `tail` Values are held only in
        // this stack frame, not on the root stack, and would otherwise be
        // swept while we're still stitching the chain together.

        // Build (0 . (1 . (2 . ... (99 . nil))))
        let mut tail = Value::nil();
        for i in (0..100i64).rev() {
            tail = heap.cons_value(Value::fixnum(i).raw(), tail.raw());
        }
        let head_ptr = tail
            .as_heap_ptr()
            .expect("cons_value must produce a heap ptr");
        let root_idx = heap.push_root(head_ptr as *const GcHeader);

        // Now it's safe to force frequent GC — only the rooted head should
        // keep the chain alive.
        heap.set_gc_threshold(1024);
        for _ in 0..2_000u64 {
            let _ = heap.cons_value(0, Value::nil().raw());
        }
        heap.collect(); // one final explicit sweep
        assert!(heap.gc_count() > 0, "GC should have triggered");

        // Walk the rooted chain and verify integrity.
        let mut current = tail;
        for expected in 0..100i64 {
            // SAFETY: the head is rooted, which transitively keeps every
            // cell reachable via cdr alive. `from_raw` on the car/cdr bits
            // decodes them as Values.
            let ptr = current
                .as_heap_ptr()
                .expect("chain element must still be a heap ptr");
            let cell = ptr as *const ConsCell;
            let (car, cdr) =
                unsafe { (Value::from_raw((*cell).car), Value::from_raw((*cell).cdr)) };
            assert_eq!(
                car.as_fixnum(),
                Some(expected),
                "car at pos {expected} lost"
            );
            current = cdr;
        }
        assert!(current.is_nil(), "chain should terminate in nil");

        heap.pop_root(root_idx);
    }

    #[test]
    fn unrooted_cons_chain_swept() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_threshold(1024);

        // Allocate 50 cells with no roots.
        let mut tail = Value::nil();
        for i in 0..50i64 {
            tail = heap.cons_value(Value::fixnum(i).raw(), tail.raw());
        }
        // `tail` is Copy; no explicit drop needed — we simply never add it
        // to the root stack, so the mark phase has nothing to visit.
        let _ = tail;

        // Force a full collection with no roots on the stack.
        heap.collect();
        assert_eq!(
            heap.bytes_allocated(),
            0,
            "all unrooted cons cells must be swept"
        );
    }

    #[test]
    fn cycle_is_collected_when_unrooted() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_threshold(1024);

        // Two cons cells that we will wire into a cdr-cycle.
        let a = heap.cons_value(Value::fixnum(1).raw(), Value::nil().raw());
        let b = heap.cons_value(Value::fixnum(2).raw(), Value::nil().raw());
        let a_ptr = a.as_heap_ptr().unwrap() as *mut ConsCell;
        let b_ptr = b.as_heap_ptr().unwrap() as *mut ConsCell;
        // SAFETY: both pointers were just returned by cons_value and are
        // live. We mutate the cdr fields directly to form a.cdr -> b,
        // b.cdr -> a.
        unsafe {
            (*a_ptr).cdr = b.raw();
            (*b_ptr).cdr = a.raw();
        }

        assert_eq!(heap.bytes_allocated(), 2 * std::mem::size_of::<ConsCell>());

        // `a` and `b` are Copy Values carrying only raw pointer bits — we
        // never add them to the root stack, so nothing keeps the cells
        // reachable from the GC's point of view.
        let _ = (a, b);

        heap.collect();
        assert_eq!(
            heap.bytes_allocated(),
            0,
            "unrooted cycle must be collected — mark phase breaks cycles via marked flag"
        );
    }

    #[test]
    fn cons_value_produces_tag_heap_ptr() {
        // Phase 3: `TAG_GC_PTR` / `is_ptr` are gone entirely.
        // `Heap::cons_value` produces `TAG_HEAP_PTR`, period.
        use crate::value::Value;
        let mut heap = Heap::new();
        let heap_cons = heap.cons_value(Value::fixnum(1).raw(), Value::fixnum(2).raw());
        assert!(heap_cons.is_heap_ptr());
    }

    // -- Phase 2f: String objects on the heap -----------------------------

    #[test]
    fn string_allocation_basic() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let val: Value = heap.string_value("hello");
        assert!(val.is_heap_ptr());

        // SAFETY: we just allocated and did not collect; the pointer is
        // live. Header tag must be String; data must round-trip.
        let ptr = val.as_heap_ptr().unwrap();
        let header = unsafe { &*(ptr as *const GcHeader) };
        assert_eq!(header.tag, ObjectTag::String);
        let obj = unsafe { &*(ptr as *const StringObject) };
        assert_eq!(&*obj.data, "hello");

        // bytes_allocated reflects the StringObject header + payload.
        assert_eq!(
            heap.bytes_allocated(),
            std::mem::size_of::<StringObject>() + "hello".len(),
        );
    }

    #[test]
    fn string_unrooted_swept() {
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        for i in 0..10 {
            let _ = heap.string_value(&format!("string {i}"));
        }
        assert!(heap.bytes_allocated() > 0);

        heap.collect();
        assert_eq!(
            heap.bytes_allocated(),
            0,
            "all unrooted strings must be swept, Box::from_raw must run"
        );
    }

    #[test]
    fn string_rooted_survives_gc() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);

        let s: Value = heap.string_value("live string");
        let root_idx = heap.root_value(s).expect("heap string must be rootable");

        // Allocate garbage and collect.
        for _ in 0..50 {
            let _ = heap.string_value("garbage");
        }
        heap.collect();

        // Only the rooted string survives.
        assert_eq!(
            heap.bytes_allocated(),
            std::mem::size_of::<StringObject>() + "live string".len(),
        );
        // SAFETY: rooted so still live.
        let ptr = s.as_heap_ptr().unwrap();
        let obj = unsafe { &*(ptr as *const StringObject) };
        assert_eq!(&*obj.data, "live string");

        heap.pop_root(root_idx);
    }

    #[test]
    fn cons_containing_string_survives_gc_rooted() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);

        // Build the string first at a "safe" moment, then fold it into
        // a cons. Root the cons; tracing must visit the string via the
        // car, preventing it from being swept.
        let s = heap.string_value("kept by cons");
        let cell = heap.cons_value(s.raw(), Value::nil().raw());
        let root_idx = heap.root_value(cell).unwrap();

        // Pile up garbage of both shapes.
        for i in 0..30 {
            let _ = heap.string_value(&format!("garbage-str-{i}"));
            let _ = heap.cons_value(Value::fixnum(i).raw(), Value::nil().raw());
        }
        heap.collect();

        // Both the cons and the string survived — verify the chain.
        let cell_ptr = cell.as_heap_ptr().unwrap() as *const ConsCell;
        let car = unsafe { Value::from_raw((*cell_ptr).car) };
        assert_eq!(
            car.raw(),
            s.raw(),
            "cons car must still point at the rooted string"
        );
        let string_ptr = s.as_heap_ptr().unwrap() as *const StringObject;
        let obj = unsafe { &*string_ptr };
        assert_eq!(&*obj.data, "kept by cons");

        heap.pop_root(root_idx);
    }

    // -- Phase 2h-2k: Vector / HashTable / BytecodeFn / Bignum -------------

    #[test]
    fn vector_allocation_and_identity() {
        use crate::object::LispObject;
        use parking_lot::Mutex;
        use std::sync::Arc;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);

        // Phase 2n: build an Arc-wrapped vector and allocate.
        let items: Vec<LispObject> = vec![LispObject::integer(1), LispObject::integer(2)];
        let arc: crate::object::SharedVec = Arc::new(Mutex::new(items));
        let vec = heap.vector_value(arc.clone());
        assert!(vec.is_heap_ptr());

        // Mutate the Arc from outside — the heap's view of the vector
        // reflects the change because both share the same Arc.
        arc.lock().push(LispObject::integer(42));

        // SAFETY: we hold `arc`, which the heap also holds; the Box
        // behind `vec` is alive.
        let obj = unsafe { &*(vec.as_heap_ptr().unwrap() as *const VectorObject) };
        assert_eq!(obj.v.lock().len(), 3);
        assert_eq!(obj.v.lock()[2], LispObject::integer(42));

        // Identity: the heap Arc and the external Arc point at the same
        // underlying Mutex.
        assert!(Arc::ptr_eq(&obj.v, &arc));
    }

    #[test]
    fn vector_unrooted_swept() {
        use crate::object::LispObject;
        use parking_lot::Mutex;
        use std::sync::Arc;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let arc: crate::object::SharedVec = Arc::new(Mutex::new(vec![LispObject::integer(1)]));
        let _ = heap.vector_value(arc);
        heap.collect();
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn hashtable_allocation_and_sweep() {
        use crate::object::{HashTableTest, LispHashTable};
        use parking_lot::Mutex;
        use std::sync::Arc;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let table: crate::object::SharedHashTable =
            Arc::new(Mutex::new(LispHashTable::new(HashTableTest::Eq)));
        let ht = heap.hashtable_value(table);
        assert!(ht.is_heap_ptr());
        assert_eq!(
            heap.bytes_allocated(),
            std::mem::size_of::<HashTableObject>()
        );
        heap.collect();
        assert_eq!(
            heap.bytes_allocated(),
            0,
            "unrooted hash table must be swept"
        );
    }

    #[test]
    fn bytecode_allocation_and_sweep() {
        use crate::object::BytecodeFunction;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let func = BytecodeFunction {
            argdesc: 0,
            bytecode: vec![],
            constants: vec![],
            maxdepth: 0,
            docstring: None,
            interactive: None,
        };
        let bc = heap.bytecode_value(func);
        assert!(bc.is_heap_ptr());
        heap.collect();
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn cons_arc_preserves_identity_across_decode() {
        // Phase 2n-cons: ConsArcCell wraps the caller's Arc; two
        // `value_to_obj`-equivalent reads return Arc clones of the
        // same inner Mutex, so mutation via one is visible via the
        // other. This test inspects the heap layer directly to make
        // the identity guarantee explicit.
        use crate::object::LispObject;
        use parking_lot::Mutex;
        use std::sync::Arc;

        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);

        let arc: crate::object::ConsCell =
            Arc::new(Mutex::new((LispObject::integer(1), LispObject::integer(2))));
        let v = heap.cons_arc_value(arc.clone());
        assert!(v.is_heap_ptr());

        // SAFETY: we hold `arc` and the heap holds a clone — the Box
        // behind `v` is alive.
        let obj = unsafe { &*(v.as_heap_ptr().unwrap() as *const ConsArcCell) };
        assert!(Arc::ptr_eq(&obj.arc, &arc));

        // Mutate through the external Arc handle; the heap sees the
        // same Mutex.
        arc.lock().0 = LispObject::integer(99);
        assert_eq!(obj.arc.lock().0, LispObject::integer(99));
    }

    #[test]
    fn cons_arc_unrooted_swept() {
        use crate::object::LispObject;
        use parking_lot::Mutex;
        use std::sync::Arc;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let arc: crate::object::ConsCell =
            Arc::new(Mutex::new((LispObject::nil(), LispObject::nil())));
        let _ = heap.cons_arc_value(arc);
        heap.collect();
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn primitive_allocation_and_sweep() {
        // Phase 2o: Primitive now lives on the heap as an owned-name
        // wrapper. This replaces the last side-table caller from the
        // main interpreter's `obj_to_value`.
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let v = heap.primitive_value("car");
        assert!(v.is_heap_ptr());

        // SAFETY: we haven't collected yet, the pointer is live.
        let obj = unsafe { &*(v.as_heap_ptr().unwrap() as *const PrimitiveObject) };
        assert_eq!(obj.name, "car");

        heap.collect();
        assert_eq!(
            heap.bytes_allocated(),
            0,
            "unrooted primitive must be swept"
        );
    }

    #[test]
    fn bignum_allocation_rooted_survives() {
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        let big = heap.bignum_value(1_i64 << 50); // beyond fixnum range
        assert!(big.is_heap_ptr());
        let root_idx = heap.root_value(big).unwrap();
        heap.collect();
        assert_eq!(heap.bytes_allocated(), std::mem::size_of::<BignumObject>());
        // SAFETY: rooted.
        let obj = unsafe { &*(big.as_heap_ptr().unwrap() as *const BignumObject) };
        assert_eq!(obj.value, 1_i64 << 50);
        heap.pop_root(root_idx);
    }

    // -- Phase 2b: GcMode + Value rooting ---------------------------------

    #[test]
    fn gc_mode_manual_suppresses_automatic_sweeps() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);
        heap.set_gc_threshold(0); // would force GC on every alloc in Auto mode

        for i in 0..200i64 {
            let _ = heap.cons_value(Value::fixnum(i).raw(), Value::nil().raw());
        }
        assert_eq!(
            heap.gc_count(),
            0,
            "Manual mode must not trigger sweeps even past the threshold"
        );
        // The un-swept cells are all still on the all_objects list.
        assert_eq!(
            heap.bytes_allocated(),
            200 * std::mem::size_of::<ConsCell>()
        );

        // An explicit collect still runs.
        heap.collect();
        assert_eq!(heap.gc_count(), 1);
        assert_eq!(
            heap.bytes_allocated(),
            0,
            "explicit collect sweeps the accumulated garbage"
        );
    }

    #[test]
    fn gc_mode_auto_still_sweeps_past_threshold() {
        use crate::value::Value;
        let mut heap = Heap::new();
        // Default mode is Auto — sanity check the old behaviour survives.
        assert_eq!(heap.gc_mode(), GcMode::Auto);
        heap.set_gc_threshold(1024);

        for i in 0..500i64 {
            let _ = heap.cons_value(Value::fixnum(i).raw(), Value::nil().raw());
        }
        assert!(
            heap.gc_count() > 0,
            "Auto mode should sweep once threshold is exceeded"
        );
    }

    #[test]
    fn root_value_returns_none_for_immediates() {
        use crate::value::Value;
        let mut heap = Heap::new();

        assert!(heap.root_value(Value::nil()).is_none());
        assert!(heap.root_value(Value::t()).is_none());
        assert!(heap.root_value(Value::fixnum(42)).is_none());
        assert!(heap.root_value(Value::float(3.14)).is_none());
        assert!(heap.root_value(Value::symbol_id(0)).is_none());
        // Root stack stays empty because nothing was pushed.
        assert!(heap.root_stack.is_empty());
    }

    #[test]
    fn root_value_keeps_heap_cons_alive_across_gc() {
        use crate::value::Value;
        let mut heap = Heap::new();
        heap.set_gc_mode(GcMode::Manual);

        let cell = heap.cons_value(Value::fixnum(7).raw(), Value::nil().raw());

        // Root the cell, then run an explicit collection. The cell must
        // survive until we pop the root.
        let root_idx = heap.root_value(cell).expect("heap cons must be rootable");
        heap.collect();
        assert_eq!(
            heap.bytes_allocated(),
            std::mem::size_of::<ConsCell>(),
            "rooted cell must not be swept"
        );
        // SAFETY: cell is still alive because we rooted it.
        let ptr = cell.as_heap_ptr().unwrap() as *const ConsCell;
        let car = unsafe { Value::from_raw((*ptr).car) };
        assert_eq!(car.as_fixnum(), Some(7));

        // Pop the root. The next collection sweeps the cell.
        heap.pop_root(root_idx);
        heap.collect();
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn gc_threshold_adapts_after_collection() {
        let mut heap = Heap::new();
        heap.set_gc_threshold(64);

        // Allocate enough to trigger GC.
        for _ in 0..100 {
            let _ = heap.cons(0, 0);
        }

        assert!(heap.gc_count() > 0);
        // After collection with no roots, threshold should be the default
        // minimum since live size is ~0.
        assert!(
            heap.gc_threshold >= DEFAULT_GC_THRESHOLD,
            "Threshold should grow to at least the default after GC",
        );
    }
}
