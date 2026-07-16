function inspectElement(element: Element) {
  return [
    element.nodeName,
    element.children.length,
    element.attributes.length,
    element.contains(element),
  ];
}

declare const html: HTMLElement;
declare const svg: SVGElement;

for (let index = 0; index < 200; index += 1) {
  inspectElement(index % 2 === 0 ? html : svg);
}
