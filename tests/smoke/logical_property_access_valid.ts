function f(): string {
  const user = { active: true };

  if (user.active && true) {
    return "ok";
  }

  return "fallback";
}
