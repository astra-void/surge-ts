function f(): void {
  let a: string | number = 1;
  if (typeof a === "number") {
    a = "x";
  }
  void a;

  let b: string | undefined = undefined;
  if (b === undefined) {
    b = "y";
  }
  void b;
}

function g(): void {
  let c: string | number = 1;
  if (typeof c === "number") {
    c = true as unknown as boolean;
  }
  void c;
}

export { f, g };
