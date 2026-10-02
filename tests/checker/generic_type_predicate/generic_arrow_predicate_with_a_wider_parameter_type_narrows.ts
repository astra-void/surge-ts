// zod's shape: a generic arrow bound to a `const`, with a wider parameter type
// than the argument (`ParseReturnType<T>` against `SyncParseReturnType<Output>`).
type INVALID = { status: "aborted" };
type DIRTY<T> = { status: "dirty"; value: T };
type OK<T> = { status: "valid"; value: T };
type Sync<T> = OK<T> | DIRTY<T> | INVALID;
type Any<T> = Sync<T> | Promise<Sync<T>>;
const isValid = <T>(x: Any<T>): x is OK<T> => (x as any).status === "valid";
export const handle = <O>(r: Sync<O>): O | null => (isValid(r) ? r.value : null);
