//! Shared bookkeeping for the "does this type hide a degradation" walkers.
//!
//! Types are DAGs: a lib interface graph such as typescript-eslint's `Scope`
//! union reaches the same expanded object from many members, so walking it as
//! a tree is exponential. A node already walked in this query contributes
//! nothing new, and a query past the work budget answers `exhausted`, which the
//! walkers read as "may contain a degradation" — that only ever suppresses a
//! comparison, never reports one.

use std::cell::{Cell, RefCell};

use surge_ts_types::Type;
use surge_ts_types::fx::FxHashSet;

const WORK_BUDGET: usize = 50_000;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static WORK: Cell<usize> = const { Cell::new(0) };
    static SEEN: RefCell<FxHashSet<usize>> = RefCell::new(FxHashSet::default());
}

pub(crate) enum Visit {
    /// Walk this node's children.
    Walk,
    /// Already walked in this query.
    Seen,
    /// The query ran out of budget.
    Exhausted,
}

/// Runs one top-level query; nested calls share its state.
pub(crate) fn query<R>(run: impl FnOnce() -> R) -> R {
    let outermost = DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        current == 0
    });
    if outermost {
        WORK.with(|work| work.set(0));
        SEEN.with(|seen| seen.borrow_mut().clear());
    }
    let result = run();
    DEPTH.with(|depth| depth.set(depth.get() - 1));
    if outermost {
        SEEN.with(|seen| seen.borrow_mut().clear());
    }
    result
}

pub(crate) fn visit(ty: &Type) -> Visit {
    let exhausted = WORK.with(|work| {
        let next = work.get() + 1;
        work.set(next);
        next > WORK_BUDGET
    });
    if exhausted {
        return Visit::Exhausted;
    }
    let key = match ty {
        Type::Object(object) => std::sync::Arc::as_ptr(&object.properties) as *const () as usize,
        Type::Union(union) => union.types().as_ptr() as usize,
        _ => return Visit::Walk,
    };
    if SEEN.with(|seen| seen.borrow_mut().insert(key)) {
        Visit::Walk
    } else {
        Visit::Seen
    }
}
