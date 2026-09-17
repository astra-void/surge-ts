declare function take(value: string): void;
declare function lookup(): string | undefined;
declare const flag: boolean;

// After a module-scope `if`, a binding is what either edge left it as.
let filled = lookup();
if (!filled) {
  filled = "fallback";
}
take(filled);

let chosen = lookup();
if (flag) {
  chosen = "yes";
} else {
  chosen = "no";
}
take(chosen);

let mixed: string | number = 1;
if (flag) {
  mixed = "a";
} else {
  mixed = "b";
}
take(mixed);

// An edge that left it unassigned keeps the declared union.
let partial = lookup();
if (flag) {
  partial = "yes";
}
take(partial);
