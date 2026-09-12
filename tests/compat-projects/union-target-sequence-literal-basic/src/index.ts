type Operation = ['+', number, number] | ['-', number] | ['++', number];

declare function run(operation: Operation): number;

export const differentArity = run(['+', 1, 2]);

export const sameArityDecidedByTheLiteral = run(['-', 2]);

type A = { kind: 'a'; n: number };
type B = { kind: 'b'; s: string };

declare function collect(values: A[] | B[]): string;

export const arrayMemberDecidedByTheLiteral = collect([
  { kind: 'b', s: 'one' },
  { kind: 'b', s: 'two' },
]);

declare function nested(input: { value: A[] | B[] }): string;

export const nestedArrayMember = nested({ value: [{ kind: 'a', n: 1 }] });
