declare const source: { present: number };

// An object literal initializer is typed by the pattern: a defaulted name it
// does not write is an optional property, not a missing one.
const { present, absent = 2 } = { present: 1 };
const { onlyDefault = 1 } = {};
const { renamed: alias = "fallback" } = {};
const { nested: { deep = 1 } = {} } = { nested: {} };
const { constDefault = 1, kept } = { kept: "x" } as const;

// The default's type is what the name holds.
const absentWrong: string = absent;
const aliasWrong: number = alias;
function body(): void {
  const { local = "x" } = {};
  const localWrong: number = local;
}

// Without a default, or without a literal to type, the name is still missing.
const { required } = source;
const { fromReference = 1 } = source;
