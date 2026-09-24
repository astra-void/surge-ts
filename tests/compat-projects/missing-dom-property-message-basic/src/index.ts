interface Element {}
interface EventTarget {}
interface HTMLElement {}
interface HTMLInputElement {}
interface HTMLElementFake {}
interface Node {
    actuallyNotTheSame: number;
}

declare const element: Element;
element.textContent;
element.focus();
element.textContent = "";
declare const html: HTMLElement;
html.textContent;
declare const input: EventTarget & HTMLInputElement;
input.value;

declare const fake: HTMLElementFake;
fake.textContent;
declare const node: Node;
node.textContent;
