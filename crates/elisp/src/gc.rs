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
    /// Prevent Send/Sync.
    _not_send: PhantomData<*mut ()>,
}

/// Default initial GC threshold (256 KiB).
const DEFAULT_GC_THRESHOLD: usize = 256 * 1024;

impl Heap {
    /// Create a new, empty heap with the default GC threshold.
    pub fn new() -> Self {
        Self {
            all_objects: ptr::null_mut(),
            bytes_allocated: 0,
            gc_threshold: DEFAULT_GC_THRESHOLD,
            gc_count: 0,
            root_stack: Vec::new(),
            _not_send: PhantomData,
        }
    }

    /// Returns `true` when the heap has exceeded its allocation threshold
    /// and a collection should be triggered.
    pub fn should_gc(&self) -> bool {
        self.bytes_allocated > self.gc_threshold
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

    /// Recursively mark an object and its children.
    ///
    /// # Safety
    /// `header` must point to a valid, heap-allocated `GcHeader`.
    unsafe fn mark_object(header: *mut GcHeader) {
        if header.is_null() {
            return;
        }
        // SAFETY: caller guarantees header is valid.
        let h = unsafe { &mut *header };
        if h.marked {
            return; // already visited — break cycles
        }
        h.marked = true;

        // TODO: recurse into children based on h.tag.
        // For example, Cons cells contain car/cdr pointers that must be marked,
        // Vectors contain element pointers, etc.
        // This will be wired up when the object layout is finalised.
    }

    // -- Sweep phase --------------------------------------------------------

    /// Walk the all_objects list and free every unmarked object.
    /// Marked objects have their flag cleared for the next cycle.
    fn sweep(&mut self) {
        let mut prev: *mut *mut GcHeader = &mut self.all_objects;

        // SAFETY: we only dereference pointers that were registered via
        // `register`, which requires them to be valid.
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
                    // Object is garbage — unlink and free.
                    let next = header.next;
                    *prev = next;

                    // TODO: actually deallocate the object backing memory.
                    // For now we just unlink it from the list. Real deallocation
                    // will be added when arena/box integration is complete.
                    // self.bytes_allocated -= object_size(header);

                    current = next;
                }
            }
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

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
}
