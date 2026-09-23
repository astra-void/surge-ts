export {};

interface Element {}
interface HTMLFooElement {}
type LiteralAlias = {};

declare const element: Element;
element.textContent;
declare const foo: HTMLFooElement;
foo.textContent;
namespace Dom {
    export interface EventTarget {}
    declare const target: EventTarget;
    target.addEventListener;
}

declare const alias: LiteralAlias;
alias.textContent;
declare const mixed: Element & { id: string };
mixed.textContent;
declare const plain: object;
plain.textContent;
