function f(): string {
  const user = { age: 1 };

  if (user.name === "Ada") {
    return "ok";
  }

  return "fallback";
}
