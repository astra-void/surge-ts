// A write is what a later read of the same element sees, which is what keeps
// `counts[key] = (counts[key] ?? 0) + 1` from leaving the next read
// possibly-undefined under `noUncheckedIndexedAccess`.
// @noImplicitAny: true
// @noUncheckedIndexedAccess: true
export function f(specifiers: string[]) {
  const counts: Record<string, number> = {};
  let max = 0;
  for (const specifier of specifiers) {
    counts[specifier] = (counts[specifier] ?? 0) + 1;
    if (counts[specifier] > max) { max = counts[specifier]; }
  }
  return max;
}
