function f(): string {
  const user = { age: 1 };

  if (user.age > 0) {
    return "ok";
  }

  return "fallback";
}
