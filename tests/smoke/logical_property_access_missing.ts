function f(): string {
  const user = { active: true };

  if (user.missing && true) {
    return "ok";
  }

  return "fallback";
}
