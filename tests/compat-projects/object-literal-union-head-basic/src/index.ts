interface Foo {
  a: string;
  b: number;
}
interface Bar {
  b: string;
}
interface Other {
  unrelated: number;
}
interface CatDog {
  cat: string;
  dog: string;
}
interface ManBearPig {
  man: string;
  bear: string;
  pig: string;
}
type Animal = CatDog | ManBearPig | Other;

declare function takesFooOrOther(value: Foo | Other): void;
declare function takesAny(value: Foo | Bar | Other): void;
declare function addToZoo(animal: Animal): void;

takesFooOrOther({ a: "", b: "" });
takesAny({ a: "", b: "" });
addToZoo({ dog: "Barky" });
addToZoo({ man: "Manny", bear: "Coffee" });
addToZoo({ cat: "Tom", dog: "Spike" });

export const declared: Animal = { dog: "Barky" };
export const fitsBar: Foo | Bar | Other = { a: "", b: "" };
