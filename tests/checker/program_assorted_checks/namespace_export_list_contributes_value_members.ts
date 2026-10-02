// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: runtime.d.ts
// `declare namespace N { export { a, b } }` re-exports enclosing declarations as
// namespace members, so `N.a` resolves off a namespace import instead of
// reporting a missing property on an empty object (Prisma's runtime
// `Extensions`).
declare function getExtensionContext<T>(that: T): T;
declare function defineExtension(args: number): string;
declare namespace Extensions {
export { defineExtension, getExtensionContext }
}
export { Extensions }
// @filename: index.ts
import * as runtime from "./runtime";
export const ctx = runtime.Extensions.getExtensionContext;
export const def = runtime.Extensions.defineExtension;
