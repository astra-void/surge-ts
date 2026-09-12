# union-target-object-literal-member-basic

An object literal against a union of object members that all declare the written
properties was evaluated context-free, so its *nested* literals widened —
`{ viz: 'timeseries', requests: [ … ] }` became `{ viz: string; requests: { … }[] }`
and was then rejected by every member. The discriminator is often not at the top
level: in ts-pattern's datadog-shaped corpus it sits one and two levels down,
inside array elements.

Two signals now pick the member, cheapest first:

- **The discriminant, for free.** A property written as a primitive literal has
  its type without any evaluation, so `viz: 'timeseries'` alone selects
  `Timeseries` out of the union. The chosen member then gives every nested literal
  its context, which is how `data_source: 'metrics'` reaches `MetricQuery` two
  levels in — `decidedByTheDiscriminant` covers that whole chain.
- **One typed property, bounded.** `{ value: [ … ] }` has no literal discriminant:
  what separates `{ value: A[] }` from `{ value: B[] }` lives inside the array. So
  a single-property literal gets one real evaluation of that property against the
  union of what the candidates declare for it, which the array-literal path then
  resolves. Restricted to one property, one level of nesting, and at most four
  candidates.

The fixture is 0/0 by design. Without the fix it reports two false `TS2345`, and
a *negative* cannot be pinned here: tsc reports a wrong nested property as
`TS2322` on the property while surge reports `TS2345` on the whole argument, a
reporting-granularity difference that predates this fixture.

**There is a cost bound in the implementation that no fixture can express.**
Typing a literal against a *wide* member is the expensive half — with no bound,
and even with every probe removed, the tanstack-query aggregate went from about
4 s to not finishing in five minutes on its query-option unions. A member with
more than 20 own properties is therefore left alone; 40 already reproduces the
blowup. The number is a measured cost bound, not a semantic rule.
