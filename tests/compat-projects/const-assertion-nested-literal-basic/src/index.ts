declare function takeTriple(triple: readonly [number, number, number]): void;
declare function takeNested(rows: readonly (readonly [number, number])[]): void;

declare function elementOf<B>(values: readonly B[]): B;

export const nestedTuple = elementOf([[1, 2, 3]] as const);
takeTriple(nestedTuple);

declare function fromCallback<A, B>(
  values: readonly A[],
  project: (value: A) => readonly B[],
): B[];

export const throughACallback = fromCallback([1, 2], (value) => [
  [value, value, value],
] as const);
takeTriple(throughACallback[0]);

export const objectInside = elementOf([{ kind: 'a', at: [1, 2] }] as const);
takeNested([objectInside.at]);
