export {};
enum AnimalType {
  cat = "cat",
  dog = "dog",
}
enum Code {
  one = 1,
}

const fromMember = `${AnimalType.cat}`;
const isCat: "cat" = fromMember;
const prefix = "x";
const joined = `${prefix}-${1}-${AnimalType.dog}-${Code.one}`;
const exact: "x-1-dog-1" = joined;
const nested = `<${`${prefix}${prefix}`}>`;
const nestedExact: "<xx>" = nested;
let widened = `${AnimalType.cat}`;
const notLiteral: "cat" = widened;
declare const open: string;
const unevaluated = `${open}!`;
const alsoNotLiteral: "!" = unevaluated;

type Animal =
  | { type: `${AnimalType.cat}`; meow: string }
  | { type: `${AnimalType.dog}`; bark: string };

function check(p: never) {
  throw new Error(String(p));
}

function byTemplateCase(animal: Animal) {
  switch (animal.type) {
    case `${AnimalType.cat}`:
      animal.meow;
      break;
    case `${AnimalType.dog}`:
      animal.bark;
      break;
    default:
      check(animal);
  }
}
