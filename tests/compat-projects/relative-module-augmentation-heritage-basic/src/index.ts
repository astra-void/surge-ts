import type { TSESTree } from './pkg';

// Inherited through `Identifier extends BaseNode`.
export function viaDerived(id: TSESTree.Identifier) {
  return id.parent.type;
}

// Through a union of derived interfaces.
export function viaUnion(node: TSESTree.Node) {
  return node.parent;
}

// The augmented member carries its declared type, not `any`.
export function keepsDeclaredType(id: TSESTree.Identifier): TSESTree.Node {
  return id.parent;
}

// Not a suppression: an ordinary mismatch is still reported.
declare const count: number;
export const bad: string = count;
