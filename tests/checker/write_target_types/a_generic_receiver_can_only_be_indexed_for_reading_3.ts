// `AccessFlags.NoIndexSignatures`: a generic receiver indexed by a concrete
// key can only be read. A generic key defers to an indexed-access type
// instead, and a literal one names a member of the constraint.
export function writeNamed<T extends { a: number }>(o: T) {
  o["a"] = 1;
}
