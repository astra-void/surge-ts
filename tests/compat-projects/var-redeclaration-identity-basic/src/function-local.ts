function f(x1: Uppercase<string>) {
  var x4: Lowercase<Uppercase<string>> = null as any;
  x1 = x4;
  x4 = x1;
  var n: number = 1;
  n = "s";
}
export {};
