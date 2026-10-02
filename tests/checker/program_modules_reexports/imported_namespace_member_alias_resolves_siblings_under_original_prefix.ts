// @noImplicitAny: true
// @filename: node_modules/lib/index.d.ts
// `import { Handler }` renames the namespace member to its bare local form, but
// its body still references siblings (`Fn`, `Ev`) that only resolve under the
// member's original `NS.` prefix. The prefix is recovered from `declared_name`
// (the qualified source name), not the bare binding. Regression for React's
// `import { MouseEventHandler } from "react"` falsely reporting TS7006 on the
// contextually-typed callback parameter.
export = NS;
declare namespace NS {
interface Ev<T> { x: T }
type Fn<E> = (e: E) => void;
type Handler<T> = Fn<Ev<T>>;
}
// @filename: src/index.ts
import { Handler } from 'lib';
const h: Handler<number> = (e) => e.x;
