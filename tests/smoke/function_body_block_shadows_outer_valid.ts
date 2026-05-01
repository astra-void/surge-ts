function f(): string {
  const value = "outer";

  {
    const value = "inner";
    return value;
  }
}
