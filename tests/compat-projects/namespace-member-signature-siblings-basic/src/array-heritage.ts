import * as ast from "../vendor/ast";

interface Boxes<T> extends ReadonlyArray<T> {
  readonly label: string;
}

export function kinds(nodes: ast.NodeList<ast.Node>) {
  const collected: number[] = [];
  nodes.forEach((node) => collected.push(node.kind));
  return collected.length === nodes.length && nodes.some((node) => node.kind > 0);
}

export function labelled(boxes: Boxes<number>) {
  return `${boxes.label}:${boxes.length}:${boxes.map((value) => value + 1).join(",")}`;
}
