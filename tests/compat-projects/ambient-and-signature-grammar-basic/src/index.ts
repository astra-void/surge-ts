export {};

declare namespace Ambient {
  let x: number;
  x = 1;
  if (x) {
  }
}
declare namespace Nested {
  declare const z: number;
  declare function f(): void;
}

interface LiteralKey {
  [key: "a"]: number;
}
interface BooleanKey {
  [key: boolean]: number;
}
interface GenericKey<T extends string> {
  [key: T]: number;
}
type TemplateKey = { [key: `a${string}`]: number };
interface Duplicated {
  [key: string]: number;
  [other: string]: number;
}
type DuplicatedNumber = {
  [key: number]: string;
  [other: number]: string;
};
class DuplicatedInClass {
  [key: string]: any;
  [other: string]: any;
}

enum Merged {
  A,
}
function Merged() {}
class Clashing {}
enum Clashing {
  B,
}
namespace Merged {}

const {};
const { a }: { a: number };
let [b]: number[];
