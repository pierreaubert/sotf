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
    /// Explicit root stack. Entries are raw pointers to GcHeaders that the
    /// mutator considers live. Managed via `RootGuard` RAII handles.
    root_stack: Vec<*const GcHeader>,
    /// Typed arena for fixed-size cons cells.
    cons_arena: Arena<ConsCell>,
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
            root_stack: Vec::new(),
            cons_arena: Arena::new(Self::CONS_PAGE_SIZE),
            _not_send: PhantomData,
        }
    }

    /// Set the GC threshold (useful for testing).
    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.gc_threshold = threshold;
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

    /// Trigger a GC cycle if the allocation threshold has been exceeded.
    fn maybe_gc(&mut self) {
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

            // Trace children based on object type.
            // Currently only Cons cells have traceable children. The car/cdr
            // fields are u64 placeholders; when they become real Values we
            // will decode them here and push GC pointers onto `work`.
            // For now, no child tracing is needed since u64 values are not
            // GC pointers.
            if hdr.tag == ObjectTag::Cons {
                // Future: decode car/cdr as Values, push heap pointers onto `work`.
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

                    // Return the slot to the appropriate arena's free list.
                    match header.tag {
                        ObjectTag::Cons => {
                            // SAFETY: the header is the first field of a
                            // ConsCell allocated from `cons_arena`, so this
                            // cast is valid.
                            let cons = current as *mut ConsCell;
                            self.cons_arena.free(cons);
                        }
                        _ => {
                            // Other object types are not yet arena-managed.
                            // Objects registered via `register` (e.g. stack-
                            // local headers in tests) are just unlinked; the
                            // caller owns the backing memory.
                        }
                    }

                    current = next;
                }
            }
        }
    }

    /// Return the size in bytes attributed to a GC object based on its tag.
    fn object_size(header: &GcHeader) -> usize {
        match header.tag {
            ObjectTag::Cons => std::mem::size_of::<ConsCell>(),
            // Other object types will get real sizes when they are
            // arena-managed. For now, externally registered objects don't
            // have their size tracked here.
            _ => 0,
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
