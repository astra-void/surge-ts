// @noPropertyAccessFromIndexSignature: true
// A base that degrades (`T & { url: string }` drops the bare type parameter)
// carries a synthetic open index. A derived interface inherits that
// openness, not a declared index signature, so an undeclared member on the
// derived type is still silent rather than a false TS4111.
type Loose<T> = T & { url: string };
interface Derived<T extends { path?: string }> extends Loose<T> { own: number }
function use<T extends { path?: string }>(o: Derived<T>) { return [o.own, o.path, o.missing]; }
