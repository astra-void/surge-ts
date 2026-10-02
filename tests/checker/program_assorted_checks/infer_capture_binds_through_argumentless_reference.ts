// @filename: node_modules/dep/index.d.ts
// `ComponentProps<typeof Component>` matches `JSXElementConstructor<infer P>`
// against a nominal reference (`ForwardRefExoticComponent<Props>`) whose own
// arguments are empty — the props live in its resolved call signature. The
// positional reference shortcut has nothing to line up there, so it must fall
// through to the structural expansion instead of leaving every capture at its
// seeded placeholder (which collapsed the whole conditional to `unknown`).
interface Exotic { (props: { checked: boolean }): string }
type Ctor<P> = (props: P) => string;
type PropsOf<T extends Ctor<any>> = T extends Ctor<infer P> ? P : {};
declare const Widget: Exotic;
export { Widget, type PropsOf };
// @filename: src/index.ts
import { Widget, type PropsOf } from 'dep';
declare const p: PropsOf<typeof Widget>;
export const ok: boolean = p.checked;
export const bad: string = p.checked;
