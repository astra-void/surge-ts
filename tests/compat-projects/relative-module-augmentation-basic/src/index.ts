import type { Spec } from './pkg/augment';
import type { Identifier } from './pkg/generated';

// Through the `export * as` namespace the augmenting file publishes.
export function viaNamespace(id: Spec.Identifier) {
  return id.parent.kind;
}

// And straight from the augmented file, which no consumer reaches by the
// augmentation's own specifier either.
export function viaDirect(id: Identifier) {
  return id.parent.kind;
}

// The augmented member carries its declared type, not `any`.
interface Local {
  parent: string;
}
export function keepsDeclaredType(id: Identifier, local: Local) {
  local.parent = id.parent.kind;
}

// Not a suppression: an ordinary mismatch is still reported.
declare const count: number;
export const bad: string = count;
