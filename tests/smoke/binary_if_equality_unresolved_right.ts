function f(value: string): string {
  if (value === missing) {
    return "ok";
  }

  return "fallback";
}
