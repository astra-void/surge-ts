// @filename: core.d.ts
// An `export =` namespace imported under a different alias binds its members
// under that alias (`react.Forward` for `React.Forward`). The interface's own
// `extends` clause names its siblings unqualified, so the sibling lookup has to
// use the *declared* namespace prefix, not the alias — resolving it under the
// alias missed the base and silently dropped its call signature (lucide's
// `import * as react from "react"` view of `ForwardRefExoticComponent`).
declare namespace React {
interface ExoticComponent { (props: number): string }
interface Forward extends ExoticComponent { tag?: string }
}
export = React;
// @filename: index.ts
import * as react from "./core";
declare const c: react.Forward;
export const r: string = c(1);
