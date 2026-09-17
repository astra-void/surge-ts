declare const named: { inner?: { deep: string } };
declare const u: unknown;
declare function unknownValue(): unknown;
declare const aaaaaaaaaaaaaaaa: { bbbbbbbbbbbbbbbb: { cccccccccccccccc: { dddddddddddddddd: { eeeeeeeeeeeeeeee: { ffffffffffffffff?: { g: string } } } } } };
declare const zzzzzzzzzzzzzzzz: { bbbbbbbbbbbbbbbb: { cccccccccccccccc: { dddddddddddddddd: { eeeeeeeeeeeeeeee?: { g: string } } } } };

// The two value keywords report as values, not as nullable places.
export const fromUndefinedKeyword = undefined.length;
export const fromNullKeyword = null.length;

// An `unknown` receiver tsc can name, and one it cannot.
export const namedUnknown = u.foo;
export const unnameableUnknown = unknownValue().foo;

// The entity name stands in only while it is under 100 bytes.
export const nameTooLong = aaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbb.cccccccccccccccc.dddddddddddddddd.eeeeeeeeeeeeeeee.ffffffffffffffff.g;
export const nameShortEnough = zzzzzzzzzzzzzzzz.bbbbbbbbbbbbbbbb.cccccccccccccccc.dddddddddddddddd.eeeeeeeeeeeeeeee.g;

// Noise: none of these may report.
export const guardedByChain = named.inner?.deep;
export const guardedByCheck = named.inner ? named.inner.deep : '';
export const wholeChainOptional = aaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbb.cccccccccccccccc.dddddddddddddddd.eeeeeeeeeeeeeeee.ffffffffffffffff?.g;
