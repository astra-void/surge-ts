interface Named { type: "named"; name: string }
interface Keyed { type: "keyed"; key: string }
type Node = Named | Keyed;

export const Utils = {
  isNamed(node: Node): node is Named {
    return node.type === "named";
  },
  isKeyed: (node: Node): node is Keyed => node.type === "keyed",
  hasName(node: Node, name: string): node is Named {
    return Utils.isNamed(node) && node.name === name;
  },
};

export function read(node: Node): string {
  if (Utils.isNamed(node)) {
    return node.name;
  }
  return node.key;
}

export function readArrow(node: Node): string {
  return Utils.isKeyed(node) ? node.key : node.name;
}

export function wrong(node: Node): string {
  if (Utils.isNamed(node)) {
    return node.key;
  }
  return "";
}
