interface Options {
  label: string;
  count?: number;
  nested: { depth: number };
}

// Destructured parameters are typed by the parameter's annotation.
function declaration({ label, count = 0 }: Options): void {
  const wrongLabel: number = label;
  const wrongCount: string = count;
}
const arrow = ({ nested: { depth } }: Options) => {
  const wrongDepth: string = depth;
};
const expression = function ([first, second]: [number, string]) {
  const wrongFirst: string = first;
  const wrongSecond: number = second;
};
class Handler {
  handle({ label }: Options): void {
    const wrongLabel: boolean = label;
  }
}
function rest({ label, ...others }: Options): void {
  const wrongNested: string = others.nested;
}

// Well-typed uses stay clean.
function clean({ label, nested: { depth } }: Options): string {
  return label + depth;
}
