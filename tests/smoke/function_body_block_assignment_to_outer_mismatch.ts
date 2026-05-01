function f(): string {
  let value = "hello";

  {
    value = 1;
  }

  return value;
}
