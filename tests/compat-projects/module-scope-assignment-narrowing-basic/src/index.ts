declare function take(value: string): void;
declare function lookup(): string | undefined;

// A module-scope assignment narrows the binding for what follows.
let assigned: string | undefined;
assigned = "value";
take(assigned);

let defaulted = lookup();
defaulted ??= "fallback";
take(defaulted);

let ored = lookup();
ored ||= "fallback";
take(ored);

let mixed: string | number = 1;
mixed = "text";
take(mixed);

// Assigning a wider value widens it again.
let widened: string | undefined = "value";
widened = lookup();
take(widened);

// A hoisted function may run before the assignment, so it reads the declared type.
let late = lookup();
late = "value";
function runsAnytime(): void {
  take(late);
}
