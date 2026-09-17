interface Shape {
  required: string;
  optional?: number;
}

// `-?` and `+?` must produce a real mapped object, not a degraded one: indexing
// the mapping by `keyof t` inside the same alias is what exposed the loss.
type MinusIndexed<t> = { [k in keyof t]-?: 'x' }[keyof t];
type PlusIndexed<t> = { [k in keyof t]+?: 'x' }[keyof t];
type PlainIndexed<t> = { [k in keyof t]: 'x' }[keyof t];

const minusIndexed: MinusIndexed<Shape> = 'x';
const plusIndexed: PlusIndexed<Shape> = undefined;
const plainIndexed: PlainIndexed<Shape> = 'x';

// `-?` clears the optional flag *and* strips `undefined` from the property.
type Req<t> = { [k in keyof t]-?: t[k] };
declare const req: Req<Shape>;
const reqRequired: string = req.required;
const reqOptional: number = req.optional;

// `+?` adds it back.
type Opt<t> = { [k in keyof t]+?: t[k] };
declare const opt: Opt<Shape>;
const optOptional: number | undefined = opt.optional;

// No modifier keeps the source's own optionality.
type Keep<t> = { [k in keyof t]: t[k] };
declare const keep: Keep<Shape>;
const keepRequired: string = keep.required;
const keepOptional: number | undefined = keep.optional;

// Controls: the mapping must not become permissive.
// @ts-expect-error - `+?` made `required` optional, so it is possibly undefined.
const plusStillChecked: string = opt.required;
// @ts-expect-error - `-?` does not invent a member the source never had.
req.absent;
// @ts-expect-error - `-?` keeps the property's real type.
const minusStillTyped: string = req.optional;
// @ts-expect-error - the mapped value type is still `'x'`, not any string.
const valueStillTyped: 'y' = minusIndexed;

export { minusIndexed, plusIndexed, plainIndexed, reqRequired, reqOptional, optOptional, keepRequired, keepOptional, plusStillChecked, minusStillTyped, valueStillTyped };
