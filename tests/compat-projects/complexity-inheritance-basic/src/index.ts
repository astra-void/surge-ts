interface Base {
  id: number;
  label: string;
}

interface Left extends Base {
  left: boolean;
}

interface Right extends Base {
  right: boolean;
}

interface Wide extends Left, Right {
  own: string;
}

interface Deep1 extends Wide {
  d1: number;
  label: "tag";
}

interface Deep2 extends Deep1 {
  d2: number;
}

interface Deep3 extends Deep2 {
  d3: number;
}

declare const deep: Deep3;

const id: number = deep.id;
const own: string = deep.own;
const overridden: "tag" = deep.label;
const d1: number = deep.d1;
const missing: number = deep.nope;
const wrong: string = deep.d3;

export { id, own, overridden, d1, missing, wrong };
