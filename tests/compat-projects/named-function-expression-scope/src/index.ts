export {};

declare function take(callback: unknown): void;

// A named function expression binds its own name in its own scope, wherever
// the expression is written.
export const returnsItself = function self(): unknown { return self; };
export const callsItself = function countdown(n: number): number { return n > 0 ? countdown(n - 1) : 0; };
export const invoked = (function immediately(): unknown { return immediately; })();
export const embedded = `abc${function inTemplate(): unknown { return inTemplate; }}def`;
export const listed = [function inArray(): unknown { return inArray; }];
export const nestedInObject = { member: function inObject(): unknown { return inObject; } };
take(function asArgument(): unknown { return asArgument; });
export const inSignature = function typed(x: typeof typed): typeof typed { return x; };

const shadowed = 1;
export const shadows = function shadowed(): number { return shadowed() + 1; };

export function outer() {
    const inner = function named(): unknown { return named; };
    return inner;
}

// The name is a function binding of the expression's own scope: not
// writable, and not visible outside it; a method's name is a property.
export const reassigns = function fixed(): void { fixed = () => {}; };
export const outside = function hidden(): void {};
hidden();
export const method = { run(): unknown { return run; } };
