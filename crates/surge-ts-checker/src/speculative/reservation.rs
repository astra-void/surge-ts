use std::hash::Hash;

use surge_ts_types::fx::FxHashMap;
use surge_ts_types::Type;

use crate::context::{
    DeclarationResolutionKey, InterfaceInstantiationKey,
    InterfaceMemberInstantiationKey, InterfaceOverloadInstantiationKey,
    StableInterfaceDeclarationId,
};

// ---------------------------------------------------------------------------
// Publisher-stamped pending reservations
// ---------------------------------------------------------------------------
//
// A replay of position `k` reads the committed store (positions `< frontier`)
// and its own private overlay. When it misses a key that an *earlier* serial
// position will publish but has not committed yet, the resolver recomputes the
// key against an incomplete view — it over-recurses into the declaration body
// and interns spurious structural sub-instantiations, enlarging the digest
// dependency graph and manufacturing false conflicts (see
// `docs/perf/TRPC-ORDERED-DELTA-REPLAY.md` §2).
//
// The reservation table lets a lookup distinguish that case from a genuine
// miss. Before the commit/replay walk, each position reserves — stamped with
// its serial position — the keys its worker log says it will publish. A replay
// at `k` that misses a key in the committed store consults the table: a Pending
// reservation owned by a position `< k` means serial checking would have seen
// that publisher's value, so the replay must *defer* (and be requeued once the
// publisher commits) rather than recompute. Reservations owned by `>= k`
// (future or self) are invisible; a Ready reservation means the value is
// already committed (the store lookup already hit); a Cancelled reservation
// means its owner turned out not to publish the key, so it is not a dependency.
//
// The schedule is derived from worker logs, so it is only a hint: an
// over-reservation makes a later replay defer unnecessarily (it re-checks after
// the publisher commits and finds a hit or a genuine miss — either correct); an
// under-reservation makes a replay compute a key fresh, which the existing
// digest-based commit validation catches and falls back to an inline recheck.
// Imprecision only costs performance, never correctness. Equality is by exact
// key (and exact arguments for the bucketed caches), never by the 64-bit
// conflict digest, so a digest collision cannot merge two distinct keys.

/// Lifecycle of one publisher's claim on a cache key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ReservationState {
    /// The owning position intends to publish this key but has not committed.
    Pending,
    /// The owning position committed; the real value is in the live cache.
    Ready,
    /// The owning position's speculative work was discarded without publishing
    /// this key, so it is not a dependency for any later replay.
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(super) struct Reservation {
    /// Serial position that owns this reservation.
    pub(super) publisher: usize,
    /// Attempt/generation that created it, so a stale reservation from a
    /// discarded attempt is distinguishable from a live one.
    pub(super) attempt: u64,
    pub(super) state: ReservationState,
    /// Commit version stamped at finalization (0 while Pending/Cancelled).
    pub(super) version: u64,
}

/// Result of a positional reservation query at replay position `k`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ReservationLookup {
    /// No earlier-position publisher reserves this key: a genuine miss (serial
    /// checking at `k` would also miss it), so the caller computes it.
    None,
    /// An earlier position holds a Pending reservation on this key: serial
    /// checking at `k` would observe that publisher's value, so the caller must
    /// defer until the publisher commits rather than recompute.
    Deferred { publisher: usize, attempt: u64 },
}

/// Earliest-Pending-publisher-below-`k` visibility over one key's reservations.
/// `Ready`/`Cancelled` never create a dependency, and a publisher `>= k`
/// (a future position, or the querying position itself) is invisible — so a
/// replay never waits on itself and never observes a future publication.
pub(super) fn reservation_visibility(
    reservations: impl IntoIterator<Item = Reservation>,
    k: usize,
) -> ReservationLookup {
    let mut best: Option<Reservation> = None;
    for reservation in reservations {
        if reservation.publisher >= k || reservation.state != ReservationState::Pending {
            continue;
        }
        best = match best {
            Some(current) if current.publisher <= reservation.publisher => Some(current),
            _ => Some(reservation),
        };
    }
    match best {
        Some(reservation) => ReservationLookup::Deferred {
            publisher: reservation.publisher,
            attempt: reservation.attempt,
        },
        None => ReservationLookup::None,
    }
}

/// Exact-key publisher reservations kept alongside the six order-visible caches.
/// Bucketed caches (generic, instantiation) match on exact argument vectors,
/// mirroring the caches' own bucket equality; the flat caches key on their
/// `Hash + Eq` instantiation keys. Reservation values are never stored here —
/// the committed value lives in the real cache; this table tracks only the
/// publisher/lifecycle needed to answer the defer-vs-compute question.
#[derive(Default)]
pub(crate) struct ReservationTable {
    pub(super) generic: FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    pub(super) instantiations: FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    pub(super) physical: FxHashMap<InterfaceInstantiationKey, Vec<Reservation>>,
    pub(super) templates: FxHashMap<StableInterfaceDeclarationId, Vec<Reservation>>,
    pub(super) methods: FxHashMap<InterfaceMemberInstantiationKey, Vec<Reservation>>,
    pub(super) overloads: FxHashMap<InterfaceOverloadInstantiationKey, Vec<Reservation>>,
    pub(super) pending: usize,
    pub(super) peak_pending: usize,
}

pub(super) fn reserve_bucketed(
    map: &mut FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
    publisher: usize,
    attempt: u64,
    pending: &mut usize,
    peak: &mut usize,
) {
    let bucket = map.entry(key.clone()).or_default();
    if bucket
        .iter()
        .any(|(existing, reservation)| existing == arguments && reservation.publisher == publisher)
    {
        return;
    }
    bucket.push((
        arguments.to_vec(),
        Reservation {
            publisher,
            attempt,
            state: ReservationState::Pending,
            version: 0,
        },
    ));
    *pending += 1;
    *peak = (*peak).max(*pending);
}

pub(super) fn query_bucketed(
    map: &FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
    k: usize,
) -> ReservationLookup {
    match map.get(key) {
        None => ReservationLookup::None,
        Some(bucket) => reservation_visibility(
            bucket
                .iter()
                .filter(|(existing, _)| existing == arguments)
                .map(|(_, reservation)| *reservation),
            k,
        ),
    }
}

pub(super) fn reserve_flat<K: Clone + Eq + Hash>(
    map: &mut FxHashMap<K, Vec<Reservation>>,
    key: &K,
    publisher: usize,
    attempt: u64,
    pending: &mut usize,
    peak: &mut usize,
) {
    let slot = map.entry(key.clone()).or_default();
    if slot
        .iter()
        .any(|reservation| reservation.publisher == publisher)
    {
        return;
    }
    slot.push(Reservation {
        publisher,
        attempt,
        state: ReservationState::Pending,
        version: 0,
    });
    *pending += 1;
    *peak = (*peak).max(*pending);
}

pub(super) fn query_flat<K: Eq + Hash>(
    map: &FxHashMap<K, Vec<Reservation>>,
    key: &K,
    k: usize,
) -> ReservationLookup {
    match map.get(key) {
        None => ReservationLookup::None,
        Some(slot) => reservation_visibility(slot.iter().copied(), k),
    }
}

pub(super) fn transition_bucketed(
    map: &mut FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    publisher: usize,
    to: ReservationState,
    version: u64,
    pending: &mut usize,
) {
    for bucket in map.values_mut() {
        for (_, reservation) in bucket.iter_mut() {
            if reservation.publisher == publisher && reservation.state == ReservationState::Pending
            {
                reservation.state = to;
                reservation.version = version;
                *pending = pending.saturating_sub(1);
            }
        }
    }
}

pub(super) fn transition_flat<K>(
    map: &mut FxHashMap<K, Vec<Reservation>>,
    publisher: usize,
    to: ReservationState,
    version: u64,
    pending: &mut usize,
) {
    for slot in map.values_mut() {
        for reservation in slot.iter_mut() {
            if reservation.publisher == publisher && reservation.state == ReservationState::Pending
            {
                reservation.state = to;
                reservation.version = version;
                *pending = pending.saturating_sub(1);
            }
        }
    }
}

#[allow(dead_code)]
impl ReservationTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reserve_generic(
        &mut self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        publisher: usize,
        attempt: u64,
    ) {
        reserve_bucketed(
            &mut self.generic,
            key,
            arguments,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_generic(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        k: usize,
    ) -> ReservationLookup {
        query_bucketed(&self.generic, key, arguments, k)
    }

    pub(crate) fn reserve_instantiation(
        &mut self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        publisher: usize,
        attempt: u64,
    ) {
        reserve_bucketed(
            &mut self.instantiations,
            key,
            arguments,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_instantiation(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        k: usize,
    ) -> ReservationLookup {
        query_bucketed(&self.instantiations, key, arguments, k)
    }

    pub(crate) fn reserve_physical(
        &mut self,
        key: &InterfaceInstantiationKey,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.physical,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_physical(
        &self,
        key: &InterfaceInstantiationKey,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.physical, key, k)
    }

    pub(crate) fn reserve_template(
        &mut self,
        key: &StableInterfaceDeclarationId,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.templates,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_template(
        &self,
        key: &StableInterfaceDeclarationId,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.templates, key, k)
    }

    pub(crate) fn reserve_method(
        &mut self,
        key: &InterfaceMemberInstantiationKey,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.methods,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_method(
        &self,
        key: &InterfaceMemberInstantiationKey,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.methods, key, k)
    }

    pub(crate) fn reserve_overload(
        &mut self,
        key: &InterfaceOverloadInstantiationKey,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.overloads,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_overload(
        &self,
        key: &InterfaceOverloadInstantiationKey,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.overloads, key, k)
    }

    /// Marks every Pending reservation owned by `publisher` as `Ready` (its
    /// value is now committed). First-writer serial semantics are preserved: an
    /// earlier publisher's reservation is untouched, and a later query resolves
    /// the committed value from the store rather than deferring.
    pub(crate) fn finalize(&mut self, publisher: usize, version: u64) {
        self.transition(publisher, ReservationState::Ready, version);
    }

    /// Marks every Pending reservation owned by `publisher` as `Cancelled`
    /// (its speculative work was discarded without publishing). Dependents that
    /// deferred on it are no longer bound to it — the requeue layer wakes them.
    pub(crate) fn cancel(&mut self, publisher: usize) {
        self.transition(publisher, ReservationState::Cancelled, 0);
    }

    pub(super) fn transition(&mut self, publisher: usize, to: ReservationState, version: u64) {
        transition_bucketed(&mut self.generic, publisher, to, version, &mut self.pending);
        transition_bucketed(
            &mut self.instantiations,
            publisher,
            to,
            version,
            &mut self.pending,
        );
        transition_flat(
            &mut self.physical,
            publisher,
            to,
            version,
            &mut self.pending,
        );
        transition_flat(
            &mut self.templates,
            publisher,
            to,
            version,
            &mut self.pending,
        );
        transition_flat(&mut self.methods, publisher, to, version, &mut self.pending);
        transition_flat(
            &mut self.overloads,
            publisher,
            to,
            version,
            &mut self.pending,
        );
    }

    /// Live Pending reservations — 0 once the walk has finalized/cancelled every
    /// position (the end-of-run leak assertion).
    pub(crate) fn pending_count(&self) -> usize {
        self.pending
    }

    /// High-water mark of concurrent Pending reservations (peak metadata depth).
    pub(crate) fn peak_pending(&self) -> usize {
        self.peak_pending
    }

    /// Drops all reservation metadata. Pending entries never survive clearing.
    pub(crate) fn clear(&mut self) {
        self.generic.clear();
        self.instantiations.clear();
        self.physical.clear();
        self.templates.clear();
        self.methods.clear();
        self.overloads.clear();
        self.pending = 0;
        self.peak_pending = 0;
    }
}

#[cfg(test)]
mod reservation_tests {
    use super::*;
    use std::sync::Arc;
    use crate::context::DeclarationNamespace;

    fn dkey(name: &str) -> DeclarationResolutionKey {
        DeclarationResolutionKey {
            file_name: Arc::from("test.ts"),
            name: Arc::from(name),
            namespace: DeclarationNamespace::Type,
            fingerprint: 0,
        }
    }

    fn deferred_to(lookup: ReservationLookup) -> Option<usize> {
        match lookup {
            ReservationLookup::Deferred { publisher, .. } => Some(publisher),
            ReservationLookup::None => None,
        }
    }

    // Property 1: an earlier Pending reservation returns Deferred.
    #[test]
    fn earlier_pending_defers() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 10);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::Deferred {
                publisher: 2,
                attempt: 10
            }
        );
    }

    // Property 2: an earlier Ready reservation is not a defer (its value is in
    // the committed store, so the store lookup — which runs first — hits).
    #[test]
    fn earlier_ready_is_hit_not_defer() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 10);
        table.finalize(2, 1);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 3: a future (publisher >= k) Pending reservation is invisible.
    #[test]
    fn future_pending_invisible() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 7, 10);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 4: a future Ready reservation is invisible too.
    #[test]
    fn future_ready_invisible() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 7, 10);
        table.finalize(7, 1);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 5: a position's own reservation (publisher == k) does not make it
    // defer on itself.
    #[test]
    fn own_reservation_no_deadlock() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 5, 10);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 6: with several earlier publishers, the earliest serial position
    // wins.
    #[test]
    fn earliest_serial_position_wins() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 4, 40);
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("A"), &[Type::Number], 3, 30);
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
    }

    // Property 7: cancelling a publisher wakes dependents — the next Pending
    // publisher (or a genuine miss) governs after cancellation.
    #[test]
    fn cancel_wakes_dependents() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("A"), &[Type::Number], 3, 30);
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
        table.cancel(2);
        // Owner 2 no longer binds; the next earlier Pending publisher governs.
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(3)
        );
        table.cancel(3);
        // No Pending publisher left below k -> genuine miss.
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 6),
            ReservationLookup::None
        );
    }

    // Property 8: finalization (Ready) preserves first-writer semantics — a
    // later query resolves from the store (None here), not a defer, and an
    // unrelated earlier Pending publisher still defers.
    #[test]
    fn ready_preserves_first_writer() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("A"), &[Type::Number], 4, 40);
        table.finalize(2, 1);
        // Earliest is now Ready (committed); publisher 4 still Pending but a
        // query at k=6 sees the committed value via the store, and the table no
        // longer defers to 2. Publisher 4 is < 6 and Pending, so it defers to 4.
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(4)
        );
        // At k=3, publisher 4 is a future position -> invisible, no defer.
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 3),
            ReservationLookup::None
        );
    }

    // Property 9: exact argument vectors distinguish bucket entries — a
    // reservation on <Number> does not defer a lookup of <String>.
    #[test]
    fn exact_arguments_distinguish_bucket_entries() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::String], 6),
            ReservationLookup::None
        );
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number, Type::String], 6),
            ReservationLookup::None
        );
    }

    // Property 10: semantic equality is by key/args, never by digest, so keys
    // that a 64-bit digest would collide do not merge. Distinct declaration keys
    // are independent regardless of any hashing.
    #[test]
    fn distinct_keys_do_not_merge() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        // A different declaration key with identical args is a separate entry.
        assert_eq!(
            table.query_generic(&dkey("B"), &[Type::Number], 6),
            ReservationLookup::None
        );
        table.reserve_instantiation(&dkey("A"), &[Type::Number], 3, 30);
        // The generic and instantiation tables are independent caches.
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
        assert_eq!(
            deferred_to(table.query_instantiation(&dkey("A"), &[Type::Number], 6)),
            Some(3)
        );
    }

    // Property 11: a replay hitting several deferred keys collects distinct
    // publishers (deduplicated).
    #[test]
    fn multiple_dependencies_deduplicate() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("B"), &[Type::Number], 2, 20);
        table.reserve_instantiation(&dkey("C"), &[Type::Number], 4, 40);
        let mut publishers = std::collections::BTreeSet::new();
        for lookup in [
            table.query_generic(&dkey("A"), &[Type::Number], 6),
            table.query_generic(&dkey("B"), &[Type::Number], 6),
            table.query_instantiation(&dkey("C"), &[Type::Number], 6),
        ] {
            if let Some(publisher) = deferred_to(lookup) {
                publishers.insert(publisher);
            }
        }
        assert_eq!(publishers.into_iter().collect::<Vec<_>>(), vec![2, 4]);
    }

    // Property 12: clearing removes all reservation metadata, including the
    // Pending count.
    #[test]
    fn clear_removes_all_metadata() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_instantiation(&dkey("B"), &[Type::Number], 3, 30);
        assert!(table.pending_count() > 0);
        table.clear();
        assert_eq!(table.pending_count(), 0);
        assert_eq!(table.peak_pending(), 0);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 6),
            ReservationLookup::None
        );
        assert_eq!(
            table.query_instantiation(&dkey("B"), &[Type::Number], 6),
            ReservationLookup::None
        );
    }

    // Flat-cache smoke: the physical instantiation table shares the visibility
    // rule (constructed via a template key, which is a plain Hash+Eq key).
    #[test]
    fn flat_template_cache_defers_and_finalizes() {
        let mut table = ReservationTable::new();
        let key = StableInterfaceDeclarationId {
            canonical_file: Arc::from("lib.d.ts"),
            declaration_start: 100,
            declaration_name: Arc::from("Array"),
            merged_fragments: Arc::from(Vec::new()),
        };
        table.reserve_template(&key, 2, 20);
        assert_eq!(deferred_to(table.query_template(&key, 6)), Some(2));
        table.finalize(2, 1);
        assert_eq!(table.query_template(&key, 6), ReservationLookup::None);
        assert_eq!(table.pending_count(), 0);
    }
}
