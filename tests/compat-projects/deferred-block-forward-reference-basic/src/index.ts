// A nested function body is deferred, so it may name a `const` declared later in
// the same block — how mutually recursive schemas are written.
interface Schema {
  parse(value: unknown): unknown;
}
declare function lazy(build: () => Schema): Schema;
declare function object(shape: Record<string, Schema>): Schema;
declare function num(): Schema;

export function build(): Schema {
  const annotated: Schema = lazy(() => object({ val: num(), b: inferred }));
  const inferred = object({ val: num(), get a() { return annotated; } });
  return inferred;
}

// Straight-line order still matters for a name that is never declared.
export function missingName(): unknown {
  return notDeclaredAnywhere;
}
