interface Procedure {
  _def: { output: unknown };
}

type Decorate<R> = {
  [K in keyof R]: R[K] extends Procedure
    ? { query: () => R[K]['_def']['output'] }
    : Decorate<R[K]>;
};

type Tree = { post: { list: { _def: { output: string[] } } } };

declare const client: Decorate<Tree>;
export const posts: number = client.post.list.query();

export const $output: unique symbol = Symbol('output');
export type $output = typeof $output;
export const $input: unique symbol = Symbol('input');
export type $input = typeof $input;

type Replace<Meta, S> = Meta extends $output
  ? 'output'
  : Meta extends $input
    ? S
    : Meta extends object
      ? { [K in keyof Meta]: Replace<Meta[K], S> }
      : Meta;

declare const meta: Replace<{ in: $input; id?: string }, number>;
export const input: string = meta.in;
export const sameSymbol: $output = $input;

const id = meta.id;
if (!id) {
  throw new Error('missing id');
}
export const checkedId: string = id;
