// @noImplicitAny: true
// @noUncheckedIndexedAccess: true
// trpc's sse `isTrackedEnvelope(value) ? value[0] : …` and drizzle's
// `Array.isArray(row) ? row[0] : row` over a type variable: the access is not
// deferred, and the narrowed operand answers the key.
type Envelope = [string, number, boolean];
declare function isEnvelope(value: unknown): value is Envelope;
export function tracked<T>(value: T) {
  if (isEnvelope(value)) {
    const id = value[0];
    const wrong: number = id;
    const [first] = value;
    const alsoWrong: number = first;
  }
}
interface Query { execute: unknown }
export function* rows<T extends Query>(mapper: (rows: unknown[][]) => T["execute"], row: unknown) {
  const mapped = mapper([row as unknown[]]);
  yield Array.isArray(mapped) ? mapped[0] : mapped;
}
export function anyArray<T>(value: T, index: number) {
  if (Array.isArray(value)) {
    const head: string = value[0];
    const at: string = value[index];
  }
}
