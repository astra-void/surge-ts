type Kind = "a" | "b" | "c";
type Shape = { tag: "circle" } | { tag: "square" };

function partial(kind: Kind): number {
  switch (kind) {
    case "a":
      return 1;
    case "b":
      return 2;
  }
}

function openString(text: string): number {
  switch (text) {
    case "a":
      return 1;
  }
}

function partialDiscriminant(shape: Shape): string {
  switch (shape.tag) {
    case "circle":
      return "round";
  }
}

function partialBoolean(flag: boolean): number {
  switch (flag) {
    case true:
      return 1;
  }
}

function exhaustive(kind: Kind): number {
  switch (kind) {
    case "a":
      return 1;
    case "b":
      return 2;
    case "c":
      return 3;
  }
}

function exhaustiveDiscriminant(shape: Shape): string {
  switch (shape.tag) {
    case "circle":
      return "round";
    case "square":
      return "flat";
  }
}

function exhaustiveBoolean(flag: boolean): number {
  switch (flag) {
    case true:
      return 1;
    case false:
      return 0;
  }
}

function withDefault(kind: Kind): number {
  switch (kind) {
    case "a":
      return 1;
    default:
      return 0;
  }
}

function admitsUndefined(kind: Kind): number | undefined {
  switch (kind) {
    case "a":
      return 1;
  }
}
