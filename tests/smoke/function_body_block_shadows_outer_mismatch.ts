function f(): string {
  const value = "outer";

  {
    const value = 1;
    return value;
  }
}
