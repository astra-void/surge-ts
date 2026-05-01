function f(value: string): string {
  if (flag && value === "hello") {
    return "ok";
  }

  return "fallback";
}
