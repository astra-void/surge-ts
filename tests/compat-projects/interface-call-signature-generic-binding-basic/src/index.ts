interface Wrap {
  <T>(value: T): T[];
}

interface WrapIn<TRoot> {
  <T>(value: T): { root: TRoot; items: T[] };
}

type WrapAlias = <T>(value: T) => T[];

declare const wrap: Wrap;
declare const wrapIn: WrapIn<number>;
declare const wrapAlias: WrapAlias;

export const direct: string = wrap('a');
export const directIn: string = wrapIn('a');
export const directAlias: string = wrapAlias('a');

declare const holder: {
  wrap: Wrap;
  wrapIn: WrapIn<number>;
  wrapAlias: WrapAlias;
  maybeWrap?: Wrap;
};

export const member: string = holder.wrap('a');
export const memberIn: string = holder.wrapIn('a');
export const memberAlias: string = holder.wrapAlias('a');
export const optionalMember: string | undefined = holder.maybeWrap?.('a');

// A builder whose argument is an object literal holding a nested call of the
// same shape: the nested one is typed by inference, which read a member's
// return type off the resolved handle and so reported the outer argument as
// unresolved.
interface Build {
  <TIn extends Record<string, unknown>>(input: TIn): { built: TIn };
}
declare const build: Build;

const nested = build({ inner: build({ leaf: 1 }) });
export const nestedLeaf: string = nested.built.inner.built.leaf;

// Binding the parameters must not make every call permissive: a correct call
// still type-checks, and an argument the signature refuses still reports.
export const wrapped: string[] = wrap('a');
export const refused: number[] = wrap('a');
