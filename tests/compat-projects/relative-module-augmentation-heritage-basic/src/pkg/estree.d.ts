import type * as Spec from './generated/spec';

// Hangs `parent` on the BASE interface; every derived node inherits it. The
// consumer never names `BaseNode`, so the merge has to happen where
// `Identifier extends BaseNode` is resolved, not only in the importer's copy.
declare module './generated/spec' {
  interface BaseNode {
    parent: Spec.Node;
  }
}

export type { Spec as TSESTree };
