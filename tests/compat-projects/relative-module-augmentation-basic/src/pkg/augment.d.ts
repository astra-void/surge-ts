import type * as Spec from './generated';

// The specifier is relative to THIS file, so no consumer writes the same
// string; the augmentation has to be filed under the file it resolves to.
declare module './generated' {
  interface Identifier {
    parent: Spec.Node;
  }
}

export * as Spec from './generated';
