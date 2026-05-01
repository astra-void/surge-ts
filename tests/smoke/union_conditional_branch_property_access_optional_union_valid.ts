function f(user: { name?: string }): string | undefined {
  return true ? user.name : undefined;
}
