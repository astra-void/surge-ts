// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: primitive.ts
// `import type * as ns` elides only the runtime binding: `typeof ns.Member`
// stays legal in type positions.
export const Root = (props: { checked: boolean }) => props.checked;
// @filename: index.ts
import type * as Primitive from "./primitive";
type RootFn = typeof Primitive.Root;
declare const root: RootFn;
export const value: boolean = root({ checked: true });
