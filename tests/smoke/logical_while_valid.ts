function f(flag: boolean, count: number): string {
  while (flag && count > 0) {
    return "ok";
  }

  return "fallback";
}
