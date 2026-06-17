use std::collections::BTreeMap;
use std::hash::Hasher;
use std::mem::MaybeUninit;
use std::sync::Arc;

use oxc_allocator::Allocator;
use typescript_rust_types::{FunctionType, ObjectProperty, ObjectType, Type};

use crate::program::{
    record_arena_declaration_key_alloc_count, record_arena_object_type_payload_alloc_count,
    record_arena_type_declaration_payload_alloc_count, record_checker_arena_alloc_count,
};

/// Program-local bump allocator for checker-owned immutable data.
///
/// Safety model:
/// - One arena instance is owned per checker run and cloned into worker
///   contexts when the checker needs to fan out read-only work.
/// - Allocation only happens while declaration payloads are being lowered.
/// - Arena-backed values are stored as raw handles in checker tables, so those
///   tables can be cloned without copying payloads.
/// - The arena is never reset while any handle is still reachable.
#[derive(Clone)]
pub(crate) struct CheckerArena {
    allocator: Arc<CheckerArenaInner>,
}

struct CheckerArenaInner {
    allocator: Allocator,
}

unsafe impl Send for CheckerArenaInner {}
unsafe impl Sync for CheckerArenaInner {}

impl CheckerArena {
    pub(crate) fn new() -> Self {
        Self {
            allocator: Arc::new(CheckerArenaInner {
                allocator: Allocator::default(),
            }),
        }
    }

    pub(crate) fn ptr_eq(&self, other: &CheckerArena) -> bool {
        Arc::ptr_eq(&self.allocator, &other.allocator)
    }

    pub(crate) fn alloc_str(&self, value: &str) -> &str {
        record_checker_arena_alloc_count();
        record_arena_declaration_key_alloc_count();
        self.allocator.allocator.alloc_str(value)
    }

    pub(crate) fn alloc_type_declaration_payload<T>(&self, value: T) -> &T {
        record_checker_arena_alloc_count();
        record_arena_type_declaration_payload_alloc_count();
        let value = self.allocator.allocator.alloc(MaybeUninit::new(value));
        unsafe { &*value.as_ptr() }
    }
}

pub(crate) fn alloc_object_type(
    properties: BTreeMap<String, ObjectProperty>,
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
