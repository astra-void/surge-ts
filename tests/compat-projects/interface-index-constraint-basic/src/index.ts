export {};

interface Numbers {
  [key: string]: number;
  text: string;
  count: number;
  maybe?: number;
  method(): void;
}
interface ByPosition {
  [index: number]: string;
  0: number;
  name: boolean;
  "1": string;
}
interface Both {
  [key: string]: string | number;
  [index: number]: string;
  1: number;
  flag: boolean;
}
interface Shapes {
  [key: string]: { a: number };
  wider: { a: number; b: string };
  other: { b: string };
}
