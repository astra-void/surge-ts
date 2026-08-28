# Member-level lazy interface expansion — design (2026-08-28)

Status: designed, Stage 1 in progress. Successor to the degraded-peel pin
(`LazyInstantiation.degraded_memo`) and the deferral tiers in
`resolve/named.rs`. Prereq for flipping `SURGE_AMBIENT_BLOCK_IMPORTS` on by
default (+36% user on tRPC today, diagnostics set-identical).

## Cost model (verified)

`resolve_interface_declaration` resolves every member annotation eagerly per
expansion. The depth cost is concentrated in method param/return annotations
(`resolve_function_type` recurses to full depth; only `ParsedType::Named` gets
the cheap lazy-reference out). Heritage's real cost is the base *peel* forcing
the base's full member resolution, not the member clone (Arc bumps). Peel-reason
counters on tRPC: `interface_method_mapping` 63k vs `own_property_mapping`
15.9k — methods carry an estimated 60–80% of member-mapping cost.

## Design

New `LazyMemberAnnotation` generalizing `LazyDeclarationAnnotation`
(cache.rs ~403): one Arc-shared `LazyMemberEnvironment` per expansion
(environment handle + creation scope + the interface's local substitution),
per-member parsed annotation + display + weak memo + check-phase-only
`degraded_memo`. Resolution mirrors `resolve_arc_inner`: recover context,
install scope, extend substitution, `resolve_parsed_type`, intern clean results
via `intern_instantiation`, pin degraded ones, `note_expansion_degradation()`
on every degraded read.

### Tier gate (what stays eager)

Primitives/literals/keywords (keeps discriminant members cheap for narrowing),
`ParsedType::Named` (already defers), anything containing `typeof` (value
tables dropped from captured environments), index/call/construct signatures
(construct sigs overload-merge), all members of non-library files
(`validate_local_type_declaration` must keep the fully-eager path — lazy-force
diagnostics are dropped with the recovered context).

### Method members (Stage 2)

The member stays a `Type::Function` SHELL (arity/variadic/required from the
parsed shape) with lazy param/return components — never a whole-member lazy ref
(overload folding, the inherited-function contamination probe, and the
heritage method/property split all match on the Function variant).

Hard constraints found:
- `merge_overload_signatures` compares param slots by equality and must NOT
  peel; distinct lazy-ref ids would widen previously-equal slots to `Any` and
  destroy the callback-slot union. Stage 2 defers components only for methods
  whose overload group has size 1 (from the declaration template); groups need
  content-addressed ids first (Stage 4).
- `contains_callable` (contextual-typing dependency classifier) has no
  `Reference` arm; replace with a syntactic classifier over the PARSED
  annotation for deferred methods — bit-identical to today's semantics because
  a named param resolving to a callable already fell through the missing arm.

### Cleanliness

A lazy member ref is NOT a degraded value — the enclosing expansion is
"structurally clean" if lowering succeeded and may be interned containing lazy
refs. Member degradation surfaces at first read through the member's own
degraded_memo and taints the *forcing* consumer's window (attribution shifts
from the declaring interface to the reader — conservative, but measure the
module-memo store-rate delta). The member no longer sets the interface's
`had_error`; blast radius bounded by the library-scope gate.

### Heritage

With lazy members the forced base expansion becomes shallow, so heritage is
affordable as-is; a member-list-without-expansion path is a Stage-4-optional
refinement (peel-reason `interface_heritage_resolution` was only 598 on tRPC).

### Display

`Type::name()` never peels a Reference; a lazy member renders as its
annotation display. Drift class: annotation text vs resolved rendering
(optional-member `| undefined` dedup, literal quoting). zod is the historical
display-drift detector; run the sweep with `--strictMessages` as the early
warning even though non-gating.

## Stages

1. **Property members** (non-`ParsedType::Function`), opt-in
   `SURGE_LAZY_IFACE_MEMBERS=1`, library-scoped files only, tier-gated.
2. **Method components**, overload-group-size-1 only, FunctionType shell
   eager, syntactic contextual classifier.
3. Default flip (escape hatch `=0`), then `SURGE_AMBIENT_BLOCK_IMPORTS`
   default as its own gated commit once ambient-on user ≈ ambient-off.
4. Content-addressed member-ref interner (member declaration id +
   substitution fingerprint + environment) → PropertyMap building becomes Arc
   bumps; then overload-group components, construct signatures.

## Risk register (byte-identity breakers)

1. Overload-merge widening / callback-slot loss — structurally unreachable
   under the group-size-1 gate; sentinel corpora ofetch + zod; debug counter
   `overload_merge_saw_reference_slot_count` must stay 0.
2. Taint-attribution shift — diff `SURGE_TRACE_HAD_ERROR` member/base sites
   A/B; watch `physical_interface_cache_reject_degradation_count` and module
   memo store rates.
3. Display drift in structural message walls — `--strictMessages` sweep per
   stage; Stage 1 excludes undefined-bearing unions in optional members.

## Success criteria

`SURGE_AMBIENT_BLOCK_IMPORTS=1` tRPC user ≈ flag-off (then flip it);
`function_type_payload_alloc_by_expansion_reason[interface_*]` falls sharply;
`lazy_member_annotation_force_count ≪ create_count` is the win proof.
