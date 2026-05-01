function f(): string {
  while (flag) {
    return "hello";
  }

  return "fallback";
}
