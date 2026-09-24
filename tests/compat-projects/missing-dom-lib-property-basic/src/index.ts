interface Element {}
interface HTMLElement {}
declare const e: Element; e.foo;
declare const h: HTMLElement; h.click();
interface Node2 {} declare const n: Node2; n.foo;
interface Node { }
declare const nd: Node; nd.bar;
declare const en: Element | Node; en.baz;
interface HTMLDivElement { x: number }
declare const d: HTMLDivElement; d.click();
export {}
