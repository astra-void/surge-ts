// @surge-compare: order
// tsc reports one error per object literal: a written property that fails is
// reported at that property, and the missing-required-property report never
// happens. The order matters beyond the message — a test writes
// `@ts-expect-error` over the property, which covers the property's line and
// not the literal's.
interface Opts { key: string; init: number; fn?: () => number }
declare function take(o: Opts): void;
declare const wrong: symbol;
take({ key: "k", fn: wrong });
