type LiteralUnion<L extends B, B extends string> = L | (B & { _?: never });
type HttpMethod = LiteralUnion<"get" | "post", string>;

declare const method: HttpMethod;
const widened: string = method;

function takesString(value: string): void {}
takesString(method);

type Brand = string & { _?: never };
declare const brand: Brand;
const asString: string = brand;
