// `AccessFlags.NoIndexSignatures`: a generic receiver indexed by a concrete
// key can only be read. A generic key defers to an indexed-access type
// instead, and a literal one names a member of the constraint.
export function write<T extends Record<string, number>>(o: T, k: string) {
  o[k] = 1;
}
