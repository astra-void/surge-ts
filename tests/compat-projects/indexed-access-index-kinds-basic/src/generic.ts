// A mapped key constrained to `keyof T` is a valid key of `T`, however the
// mapping renames it.
type Getters<T> = { [P in keyof T & string as `get${Capitalize<P>}`]: () => T[P] };
type ShapeGetters = Getters<{ name: string }>;
const getters: ShapeGetters = { getName: () => "text" };
const badGetters: ShapeGetters = { getName: () => 1 };

// `keyof T` is a generic index even where surge cannot compute it.
type Without<T, K extends keyof T> = Pick<
    T,
    ({ [P in keyof T]: P } & { [P in K]: never } & { [x: string]: never })[keyof T]
>;

// An instantiation re-reads the access without reporting it: `[]` has no
// element 0, so the false branch reads `undefined`.
type First<T extends readonly unknown[]> =
    T extends readonly [unknown, ...unknown[]] ? T[0] : T[0] | undefined;
type NoFirst = First<[]>;
type OneFirst = First<[string]>;
const noFirst: NoFirst = undefined;
const oneFirst: OneFirst = "text";
const badOneFirst: OneFirst = 1;

// A key that is not known to be a key of the type parameter is an error.
type Direct<T> = T["name"];
