// A write whose target type surge could not resolve reports nothing — the
// value carries no contextual type there, so what the evaluation would say
// about it (an implicit-`any` parameter, say) is surge's gap and not the
// source's. The narrowing the write installs still happens, which is what the
// reads after it depend on.
export function f(bag: { patterns?: string[] }) {
  bag.patterns = [];
  return bag.patterns.length;
}
