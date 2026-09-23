type Empty = { kind: "empty"; init: null };
type Filled = { kind: "filled"; init: string | null };

export function viaAnd(node: Empty | Filled) {
  return node.init !== null && node.init.length;
}

export function viaIf(node: Empty | Filled) {
  if (node.init !== null) {
    return node.init.length;
  }
  return 0;
}

export function wrong(node: Empty | Filled) {
  return node.init === null && node.init.length;
}
