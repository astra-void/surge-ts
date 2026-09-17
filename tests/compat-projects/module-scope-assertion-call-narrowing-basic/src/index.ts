declare function assertIsString(value: unknown): asserts value is string;
declare function take(value: string): void;
declare const checks: {
  isString(value: unknown): asserts value is string;
  truthy(value: unknown): asserts value;
};
declare const first: unknown;
declare const second: unknown;
declare const third: string | null;
declare const unchecked: string | null;

// A module-scope assertion call narrows its argument for the rest of the module.
assertIsString(first);
take(first);
checks.isString(second);
take(second);
checks.truthy(third);
take(third);

// Nothing asserted, nothing narrowed.
take(unchecked);
