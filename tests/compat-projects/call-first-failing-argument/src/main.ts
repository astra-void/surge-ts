declare function pair(a: number, b: { y: string }): void;
declare function triple(a: number, b: string[], c: () => string): void;

pair("x", { y: 1 });
pair(1, { y: 1 });
triple("x", [1], () => 1);
triple(1, ["a"], () => 1);
triple(1, [2], () => {
  const inner: number = "not a number";
  return "ok";
});
