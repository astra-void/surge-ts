use std::hash::Hasher;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use oxc_allocator::Allocator;
use surge_ts_types::{FunctionType, ObjectType, PropertyMap, Type};

use crate::program::{
    record_arena_declaration_key_alloc_count, record_arena_object_type_payload_alloc_count,
    record_arena_type_declaration_payload_alloc_count, record_checker_arena_alloc_count,
};

/// Program-local bump allocator for checker-owned immutable data.
///
/// Safety model — these invariants are what justify the `unsafe impl
/// Send/Sync` below, since the underlying `oxc_allocator::Allocator` is a
/// non-thread-safe bump allocator whose `alloc(&self)` mutates a cursor
/// through interior mutability:
/// - All allocation happens on the thread that created the arena (the
///   single-threaded binding/collection phases). Debug builds assert this on
///   every allocation.
/// - Before the check phase fans out to worker threads, every arena reachable
///   by workers is [`freeze`](Self::freeze)d; allocation after freeze panics
///   deterministically instead of racing. Worker contexts therefore only ever
///   *read* arena memory, which is safe to share: bump memory is append-only
///   and payloads are write-once at insert.
/// - Arena-backed values are stored as raw handles in checker tables, so those
///   tables can be cloned without copying payloads or allocating.
/// - The arena is never reset while any handle is still reachable.
#[derive(Clone)]
pub(crate) struct CheckerArena {
    allocator: Arc<CheckerArenaInner>,
}

struct CheckerArenaInner {
    allocator: Allocator,
    frozen: AtomicBool,
    /// Destructors for payloads bump-allocated into `allocator`. The bump
    /// allocator frees its chunks without running `Drop`, which would leak
    /// every payload's owned heap (declaration name/file strings, the
    /// `Arc<InterfaceBody>`/`Arc<TypeAliasBody>` refcounts, resolution-scope
    /// `Arc`s) for the process lifetime. Each payload registers its typed
    /// `drop_in_place` here; they run exactly once, when the last arena handle
    /// drops — the same point the payload memory itself is released, and
    /// after which no `TypeDeclarationHandle` can exist by the safety model.
    pending_drops: std::sync::Mutex<Vec<PendingDrop>>,
    #[cfg(debug_assertions)]
    owner: std::thread::ThreadId,
}

struct PendingDrop {
    ptr: *mut (),
    drop_fn: unsafe fn(*mut ()),
}

// Registered pointers target arena payloads whose types are `Send` (checker
// declaration payloads); the list is only drained on the final handle drop.
unsafe impl Send for PendingDrop {}

impl Drop for CheckerArenaInner {
    fn drop(&mut self) {
        let pending = std::mem::take(
            self.pending_drops
                .get_mut()
                .unwrap_or_else(|error| error.into_inner()),
        );
        for entry in pending {
            // Safety: each pointer was registered by
            // `alloc_type_declaration_payload` for a fully-initialized payload
            // in this arena, is dropped exactly once, and the arena memory it
            // points into is still alive until this struct's `allocator` field
            // drops after this loop.
            unsafe { (entry.drop_fn)(entry.ptr) }
        }
    }
}

unsafe impl Send for CheckerArenaInner {}
unsafe impl Sync for CheckerArenaInner {}

impl CheckerArena {
    pub(crate) fn new() -> Self {
        Self {
            allocator: Arc::new(CheckerArenaInner {
                allocator: Allocator::default(),
                frozen: AtomicBool::new(false),
                pending_drops: std::sync::Mutex::new(Vec::new()),
                #[cfg(debug_assertions)]
                owner: std::thread::current().id(),
            }),
        }
    }

    pub(crate) fn ptr_eq(&self, other: &CheckerArena) -> bool {
        Arc::ptr_eq(&self.allocator, &other.allocator)
    }

    /// Permanently disable allocation through every clone of this arena handle.
    /// Called before the check phase fans out to worker threads: existing
    /// payloads stay valid and readable, but a late allocation — which would
    /// race on the non-thread-safe bump cursor — panics instead.
    pub(crate) fn freeze(&self) {
        self.allocator.frozen.store(true, Ordering::Release);
    }

    fn assert_allocatable(&self) {
        assert!(
            !self.allocator.frozen.load(Ordering::Relaxed),
            "CheckerArena: allocation after freeze; arenas shared with check-phase \
             workers are read-only once the parallel fan-out starts"
        );
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            std::thread::current().id(),
            self.allocator.owner,
            "CheckerArena: allocation from a thread other than the creating thread; \
             the bump allocator is not thread-safe"
        );
    }

    pub(crate) fn alloc_str(&self, value: &str) -> &str {
        self.assert_allocatable();
        record_checker_arena_alloc_count();
        record_arena_declaration_key_alloc_count();
        self.allocator.allocator.alloc_str(value)
    }

    pub(crate) fn alloc_type_declaration_payload<T: Send>(&self, value: T) -> &T {
        self.assert_allocatable();
        record_checker_arena_alloc_count();
        record_arena_type_declaration_payload_alloc_count();
        let value = self.allocator.allocator.alloc(MaybeUninit::new(value));
        let ptr = value.as_mut_ptr();
        if std::mem::needs_drop::<T>() {
            unsafe fn drop_payload<T>(ptr: *mut ()) {
                unsafe { std::ptr::drop_in_place(ptr.cast::<T>()) }
            }
            self.allocator
                .pending_drops
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(PendingDrop {
                    ptr: ptr.cast(),
                    drop_fn: drop_payload::<T>,
                });
        }
        unsafe { &*ptr }
    }
}

pub(crate) fn alloc_object_type(
    properties: PropertyMap,
    string_index_type: Option<Type>,
) -> ObjectType {
    record_checker_arena_alloc_count();
    record_arena_object_type_payload_alloc_count();
    ObjectType::new(properties, string_index_type)
}

pub(crate) fn alloc_function_type(
    parameters: Vec<Type>,
    return_type: Type,
    is_variadic: bool,
    required_parameter_count: usize,
) -> FunctionType {
    record_checker_arena_alloc_count();
    FunctionType::new(
        parameters,
        return_type,
        is_variadic,
        required_parameter_count,
    )
}

impl std::fmt::Debug for CheckerArena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckerArena").finish_non_exhaustive()
    }
}

impl Default for CheckerArena {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ArenaStr {
    ptr: *const u8,
    len: usize,
}

impl ArenaStr {
    pub(crate) fn new(value: &str, arena: &CheckerArena) -> Self {
        let value = arena.alloc_str(value);
        Self {
            ptr: value.as_ptr(),
            len: value.len(),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        unsafe {
            let bytes = std::slice::from_raw_parts(self.ptr, self.len);
            std::str::from_utf8_unchecked(bytes)
        }
    }
}

impl std::fmt::Debug for ArenaStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ArenaStr").field(&self.as_str()).finish()
    }
}

impl PartialEq for ArenaStr {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ArenaStr {}

impl std::hash::Hash for ArenaStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.as_str(), state);
    }
}

impl std::fmt::Display for ArenaStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ArenaStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for ArenaStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

// The string points into the arena and is only read after insertion.
unsafe impl Send for ArenaStr {}
unsafe impl Sync for ArenaStr {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct CountedPayload {
        _heap: String,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for CountedPayload {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn payload_destructors_run_once_when_the_last_handle_drops() {
        let drops = Arc::new(AtomicUsize::new(0));
        let arena = CheckerArena::new();
        let clone = arena.clone();
        for i in 0..3 {
            let _ = arena.alloc_type_declaration_payload(CountedPayload {
                _heap: format!("payload-{i}"),
                drops: drops.clone(),
            });
        }
        drop(arena);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(clone);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn arc_owning_payload_releases_its_reference() {
        let shared = Arc::new("shared".to_string());
        let arena = CheckerArena::new();
        let _ = arena.alloc_type_declaration_payload(shared.clone());
        assert_eq!(Arc::strong_count(&shared), 2);
        drop(arena);
        assert_eq!(Arc::strong_count(&shared), 1);
    }

    #[test]
    fn nested_payload_drops_every_element() {
        let drops = Arc::new(AtomicUsize::new(0));
        let arena = CheckerArena::new();
        let _ = arena.alloc_type_declaration_payload(vec![
            CountedPayload {
                _heap: "a".to_string(),
                drops: drops.clone(),
            },
            CountedPayload {
                _heap: "b".to_string(),
                drops: drops.clone(),
            },
        ]);
        drop(arena);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn trivially_droppable_payloads_register_no_destructors() {
        let arena = CheckerArena::new();
        let _ = arena.alloc_type_declaration_payload(7u64);
        let _ = arena.alloc_type_declaration_payload([0usize; 4]);
        let _ = arena.alloc_str("plain str");
        assert_eq!(
            arena.allocator.pending_drops.lock().unwrap().len(),
            0,
            "non-Drop payloads must not carry destructor metadata"
        );
    }

    #[test]
    fn zero_sized_drop_payload_runs_exactly_once() {
        static ZST_DROPS: AtomicUsize = AtomicUsize::new(0);
        struct ZstDrop;
        impl Drop for ZstDrop {
            fn drop(&mut self) {
                ZST_DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }
        let arena = CheckerArena::new();
        let _ = arena.alloc_type_declaration_payload(ZstDrop);
        drop(arena);
        assert_eq!(ZST_DROPS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn allocation_works_during_construction() {
        let arena = CheckerArena::new();
        let s = arena.alloc_str("hello");
        assert_eq!(s, "hello");
        let payload = arena.alloc_type_declaration_payload(vec![1u32, 2, 3]);
        assert_eq!(payload, &[1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "allocation after freeze")]
    fn allocation_after_freeze_panics() {
        let arena = CheckerArena::new();
        let _ = arena.alloc_str("before freeze");
        arena.freeze();
        let _ = arena.alloc_str("after freeze");
    }

    #[test]
    #[should_panic(expected = "allocation after freeze")]
    fn freeze_applies_to_every_clone() {
        let arena = CheckerArena::new();
        let clone = arena.clone();
        arena.freeze();
        let _ = clone.alloc_str("after freeze via clone");
    }

    #[test]
    fn frozen_arena_payloads_readable_from_many_threads() {
        let arena = CheckerArena::new();
        let strings: Vec<ArenaStr> = (0..64)
            .map(|i| ArenaStr::new(&format!("payload-{i}"), &arena))
            .collect();
        arena.freeze();

        std::thread::scope(|scope| {
            for _ in 0..4 {
                let arena = arena.clone();
                let strings = &strings;
                scope.spawn(move || {
                    let _keeps_alive = arena;
                    for (i, s) in strings.iter().enumerate() {
                        assert_eq!(s.as_str(), format!("payload-{i}"));
                    }
                });
            }
        });
    }

    // Only debug builds carry the creating-thread assertion.
    #[cfg(debug_assertions)]
    #[test]
    fn cross_thread_allocation_panics_in_debug_builds() {
        let arena = CheckerArena::new();
        let result = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _ = arena.alloc_str("allocated off the creating thread");
                })
                .join()
        });
        assert!(result.is_err(), "expected cross-thread allocation to panic");
    }
}
