declare function constrained<T extends string>(items: T[]): T;
declare function unconstrained<T>(items: T[]): T;
declare function numeric<T extends number>(items: T[]): T;
declare function keyed<T extends string>(arg: { keys: T[] }): T[];
declare function guarded<T extends string>(items: T[], one: NoInfer<T>): T;

export function elements(): void {
  const a: "a" | "b" = constrained(["a", "b"]);
  const b: "zz" = constrained(["a", "b"]);
  const c: "a" | "b" = unconstrained(["a", "b"]);
  const d: 1 | 2 = numeric([1, 2]);
  const e: 9 = numeric([1, 2]);
  const f: ("a" | "b")[] = keyed({ keys: ["a", "b"] });
  guarded(["a", "b"], "a");
  guarded(["a", "b"], "c");
}
