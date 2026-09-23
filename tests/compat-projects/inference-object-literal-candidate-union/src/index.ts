declare function same<T>(a: T, b: T): T;
declare function same3<T>(a: T, b: T, c: T): T;
declare function same4<T>(a: T, b: T, c: T, d: T): T;
declare function each3<T>(a: T, b: T, c: T): void;
declare function orElse<T>(value: T | undefined, fallback: T): T;

// Object and array literal candidates are unioned before the common supertype
// is taken, with `null` and `undefined` set aside and added back afterwards.
each3(undefined, { x: 6, z: 1 }, { x: 6, y: "" });
each3({ x: 6, z: 1 }, null, { x: 6, y: "" });
each3({ a: 1 }, { b: "" }, { c: true });
same4(undefined, { a: 1 }, null, { b: "" });
same([{ a: 1 }], [{ b: 2 }]);

// The union widens with its members normalized: a property a sibling literal
// writes becomes an optional `undefined` member.
const joined = same3(undefined, { x: 6, z: 1 }, { x: 6, y: "" });
const joinedY: string | undefined = joined?.y;
const joinedAll: { x: number; z: number; y?: undefined } | { x: number; y: string; z?: undefined } | undefined = joined;
let pair = same({ a: 1 }, { b: "" });
pair = { a: 2 };
pair = { b: "c" };

// Candidates that are not literals compete as themselves: the inferred type is
// the leftmost candidate no later one is a supertype of.
interface Named { id: number; name: string; tags?: string[] }
interface Other { other: string }
declare const named: Named;
declare const maybeNamed: Named | undefined;
declare const other: Other;
const kept = orElse(maybeNamed, { id: 2, name: "b" });
const keptTags: string[] | undefined = kept.tags;
same(named, other);
same(named, { id: 1, name: "a", extra: true });
same({ other: "" }, named);
same3(named, undefined, { id: 1, name: "a", extra: true });
