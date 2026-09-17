interface Circle {
  kind: "circle";
  radius: number;
  shared: string;
}
interface Square {
  kind: "square";
  side: number;
  shared: string;
}

type ShapeKey = keyof (Circle | Square);
const radiusKey: ShapeKey = "radius";
const kindKey: ShapeKey = "kind";

type Disjoint = keyof ({ a: 1 } | { b: 2 });
const disjoint: Disjoint = "a";

type WithIndex = keyof ({ x: 1; y: 2 } | { [key: string]: 0 });
const indexedKey: WithIndex = "x";
const indexedMissing: WithIndex = "z";

declare const partial: Partial<Circle | Square> | null;
if (partial?.kind === "circle") {
  const radius: string = partial.radius;
}

const partialSquare: Partial<Circle | Square> = { side: 1 };
const partialNothing: Partial<Circle | Square | undefined> = undefined;
