type A = { value: string };
type B = { value: number };
type AB = A & B;

declare const value: AB;
const read = value.value;
