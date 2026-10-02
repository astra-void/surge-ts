// @noUncheckedIndexedAccess: true
interface Checker<T> { check(v: any): v is T } interface Id { name: string } interface D { id: Id | number; init: string } declare const isId: Checker<Id>; export function f(ds: D[]) { const d = ds[0]; if (isId.check(d.id) && d.id.name === 'x') { d.init; } }
