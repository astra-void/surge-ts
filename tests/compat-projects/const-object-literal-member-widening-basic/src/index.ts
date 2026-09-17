const flat = { count: 1, label: "x", enabled: true };
const literalCount: { count: 1 } = flat;
const literalLabel: { label: "x" } = flat;

const nested = { inner: { value: "q" } };
const literalNested: { inner: { value: "q" } } = nested;

function local() {
  const scoped = { key: 2 };
  const literalKey: { key: 2 } = scoped;
}

const contextual: { count: 1 } = { count: 1 };

const asserted = { count: 1 } as const;
const literalAsserted: { readonly count: 1 } = asserted;

const memberAssertion = { count: 1 as const };
const literalMember: { count: 1 } = memberAssertion;

const widenedOk: { count: number; label: string } = flat;
