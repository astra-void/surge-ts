use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use surge_ts_types::fx::{FxHashMap, FxHashSet, FxHasher};
use surge_ts_types::{FunctionType, Type};

/// Display-sensitive, sharing-insensitive fingerprint of a type's rendered
/// prefix: hashes discriminants, literals, reference display names, object
/// alias/property names — everything diagnostic text can show — over a
/// budget-bounded tree walk (no pointer memo, so two equal-display values hash
/// equal regardless of internal Arc sharing).
pub(crate) fn display_type_fingerprint(ty: &Type) -> u64 {
    let mut hasher = FxHasher::default();
    let mut budget = 500_000usize;
    display_fingerprint_walk(ty, &mut hasher, &mut budget);
    hasher.finish()
}

pub(crate) fn display_function_fingerprint(function: &FunctionType) -> u64 {
    let mut hasher = FxHasher::default();
    let mut budget = 500_000usize;
    display_fingerprint_function(function, &mut hasher, &mut budget);
    hasher.finish()
}

thread_local! {
    /// Memoized subtree fingerprints for the two payload kinds the walk
    /// re-hashes millions of times (property maps and function payloads),
    /// keyed on the payload allocation address. Each entry pins its
    /// allocation with a `Weak`, so the address can never be reused while the
    /// entry lives — the pointer key is identity-safe. The values are derived
    /// purely from content, so two equal-display payloads in DIFFERENT
    /// allocations still compute equal fingerprints and the walk's
    /// sharing-insensitive contract holds.
    static DISPLAY_FP_PROPERTY_MEMO: RefCell<
        FxHashMap<usize, (std::sync::Weak<surge_ts_types::PropertyMap>, u64)>,
    > = RefCell::new(FxHashMap::default());
    static DISPLAY_FP_FUNCTION_MEMO: RefCell<
        FxHashMap<usize, (std::sync::Weak<surge_ts_types::FunctionTypePayload>, u64)>,
    > = RefCell::new(FxHashMap::default());
    /// Payload addresses whose fingerprint is being computed on the current
    /// stack. A self-referential payload graph (an object embedding itself
    /// without a reference indirection) was previously bounded by the shared
    /// walk budget; with per-payload budgets the re-entry must short-circuit.
    static DISPLAY_FP_IN_PROGRESS: RefCell<FxHashSet<usize>> = RefCell::new(FxHashSet::default());
}

/// Wholesale-clear bound: the memos pin one shallow allocation per entry, so a
/// runaway program (or many programs in one test process) must not accumulate
/// unboundedly.
pub(super) const DISPLAY_FP_MEMO_CAP: usize = 262_144;

pub(super) fn memoized_subtree_fingerprint<T>(
    memo: &'static std::thread::LocalKey<RefCell<FxHashMap<usize, (std::sync::Weak<T>, u64)>>>,
    address: usize,
    pin: impl FnOnce() -> std::sync::Weak<T>,
    compute: impl FnOnce() -> u64,
) -> u64 {
    if let Some(fingerprint) = memo.with(|memo| memo.borrow().get(&address).map(|(_, fp)| *fp)) {
        return fingerprint;
    }
    let cycled = DISPLAY_FP_IN_PROGRESS.with(|set| !set.borrow_mut().insert(address));
    if cycled {
        // Re-entry on the same payload: hash a stable marker instead of
        // recursing. The shared-budget walk used to truncate such graphs
        // arbitrarily; a fixed marker is at least deterministic.
        return 0x9e3779b97f4a7c15;
    }
    let fingerprint = compute();
    DISPLAY_FP_IN_PROGRESS.with(|set| {
        set.borrow_mut().remove(&address);
    });
    memo.with(|memo| {
        let mut memo = memo.borrow_mut();
        if memo.len() >= DISPLAY_FP_MEMO_CAP {
            memo.clear();
        }
        memo.insert(address, (pin(), fingerprint));
    });
    fingerprint
}

pub(super) fn display_fingerprint_walk(ty: &Type, hasher: &mut impl Hasher, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    std::mem::discriminant(ty).hash(hasher);
    match ty {
        Type::StringLiteral(value) => value.hash(hasher),
        Type::NumberLiteral(value) => value.value.hash(hasher),
        Type::BooleanLiteral(value) => value.hash(hasher),
        Type::Array(element) => display_fingerprint_walk(element, hasher, budget),
        Type::Tuple(elements) => {
            for element in elements {
                display_fingerprint_walk(element, hasher, budget);
            }
        }
        Type::Union(union) => {
            for member in union.types() {
                display_fingerprint_walk(member, hasher, budget);
            }
        }
        Type::Function(function) => display_fingerprint_function(function, hasher, budget),
        Type::Object(object) => {
            object.alias_name.hash(hasher);
            let properties = &object.properties;
            let fingerprint = memoized_subtree_fingerprint(
                &DISPLAY_FP_PROPERTY_MEMO,
                Arc::as_ptr(properties) as usize,
                || Arc::downgrade(properties),
                || {
                    let mut sub_hasher = FxHasher::default();
                    let mut sub_budget = 500_000usize;
                    for (name, property) in properties.iter() {
                        name.hash(&mut sub_hasher);
                        property.is_optional().hash(&mut sub_hasher);
                        display_fingerprint_walk(&property.ty, &mut sub_hasher, &mut sub_budget);
                        if sub_budget == 0 {
                            break;
                        }
                    }
                    sub_hasher.finish()
                },
            );
            hasher.write_u64(fingerprint);
        }
        Type::Reference(reference) => {
            // The display alone is not an identity: two files each declaring
            // `type Context = …` render alike, and a memo keyed without the id
            // handed one file's expansion to the other.
            reference.id.hash(hasher);
            reference.display.hash(hasher);
            for argument in reference.arguments.iter() {
                display_fingerprint_walk(argument, hasher, budget);
            }
        }
        _ => {}
    }
}

pub(super) fn display_fingerprint_function(
    function: &FunctionType,
    hasher: &mut impl Hasher,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    let fingerprint = memoized_subtree_fingerprint(
        &DISPLAY_FP_FUNCTION_MEMO,
        function.payload_address(),
        || function.payload_weak(),
        || {
            let mut sub_hasher = FxHasher::default();
            let mut sub_budget = 500_000usize;
            for parameter in function.parameters() {
                display_fingerprint_walk(parameter, &mut sub_hasher, &mut sub_budget);
                if sub_budget == 0 {
                    break;
                }
            }
            display_fingerprint_walk(function.return_type(), &mut sub_hasher, &mut sub_budget);
            sub_hasher.finish()
        },
    );
    hasher.write_u64(fingerprint);
}
