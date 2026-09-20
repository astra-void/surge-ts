export interface Mutators<S, A> {}
export type MutatorId = keyof Mutators<unknown, unknown>;

export type Apply<S, Ms> = number extends Ms["length" & keyof Ms]
  ? S
  : Ms extends []
    ? S
    : Ms extends [[infer Mi, infer Ma], ...infer Mrs]
      ? Apply<Mutators<S, Ma>[Mi & MutatorId], Mrs>
      : never;

export type Reverse<Acc extends Array<unknown>, Rest> = Rest extends [
  infer Head,
  ...infer Tail,
]
  ? Reverse<[Head, ...Acc], Tail>
  : Acc;

declare const reversed: Reverse<[], [1, 2, 3]>;
export const reversedMismatch: number = reversed;
