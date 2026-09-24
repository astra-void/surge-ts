interface Box<V> { value: V }
type Key = readonly unknown[];
type Fn<T = unknown, K extends Key = Key> = (key: K) => T;
interface Options<TFnData = unknown, TKey extends Key = Key> {
    queryKey?: TKey;
    queryFn?: Fn<TFnData, TKey>;
}

// An `any` check type reaches both branches.
type Get<T> = T extends { a: infer A } ? Box<A> : "no";
type Nest<T> = T extends string ? "s" : T extends number ? "n" : "o";
type Direct = any extends string ? 1 : 2;

// Only the true branch when the extends type is `any` or `unknown`; a naked
// capture infers `any` itself.
type Top<T> = T extends unknown ? "yes" : "no";
type Inf<T> = T extends infer U ? [U] : never;

// A capture with no candidate is its constraint: written, or implied by its
// position.
type Constrained<T> = T extends { a: infer U extends string } ? U : never;
type Params<T> = T extends (...args: infer P) => any ? P : never;
type FromOptions<T> = T extends { queryFn?: Fn<infer D, infer K> } ? Options<D, K> : never;

// The idioms that must not read `any` as a match.
type IsAny<T> = 0 extends 1 & T ? true : false;
type IsNever<T> = [T] extends [never] ? true : false;

const get: Get<any> = Symbol();
const nest: Nest<any> = Symbol();
const direct: Direct = Symbol();
const topAny: Top<any> = Symbol();
const inf: Inf<any> = Symbol();
const constrained: Constrained<any> = Symbol();
const params: Params<any> = Symbol();
declare const fromOptions: FromOptions<any>;
const key: Key | undefined = fromOptions.queryKey;
const badKey: number = fromOptions.queryKey;
const isAny: IsAny<any> = true;
const notAny: IsAny<string> = false;
const neverAny: IsNever<any> = false;
const isNever: IsNever<never> = true;
const badIsAny: IsAny<any> = false;
const badIsNever: IsNever<any> = true;

// `any` matches no template text: a template of placeholders alone infers
// `never` for them; any other template leaves its captures their implied
// `string`.
type Starred<T> = T extends `*${infer S}*` ? S : never;
type Whole<T> = T extends `${infer S}` ? S : never;
const starred: Starred<any> = Symbol();
const whole: Whole<any> = Symbol();
