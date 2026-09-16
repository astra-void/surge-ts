declare const maybe: string | undefined;
declare const holder: { value?: string };
declare function lookup(): string | undefined;
let slot: string | undefined;

// A left operand whose syntax is never nullish makes the right one unreachable.
const fromObject = {} ?? maybe;
const fromString = "text" ?? maybe;
const fromArray = [1] ?? maybe;
const fromNot = !maybe ?? "fallback";
const fromTemplate = `text` ?? "fallback";
const fromArithmetic = 1 + 1 ?? 2;
const fromVoid = void 0 ?? "fallback";

// One that is always nullish is reported too.
const fromNull = null ?? maybe;
const fromUndefined = undefined ?? maybe;
const nestedNull = (null ?? undefined) ?? "fallback";
const chainedToNull = maybe ?? null ?? "fallback";

// References, calls, and value-dependent operators can be either.
const fromReference = maybe ?? "fallback";
const fromProperty = holder.value ?? "fallback";
const fromElement = holder["value"] ?? "fallback";
const fromCall = lookup() ?? "fallback";
const fromAssertion = (maybe as string) ?? "fallback";
const fromOr = (maybe || null) ?? "fallback";
const fromBranches = (maybe ? null : "text") ?? "fallback";
const fromAssignment = (slot = maybe) ?? "fallback";
