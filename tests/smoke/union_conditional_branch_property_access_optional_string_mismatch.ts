function f(user: { name?: string }): string {
  return true ? user.name : undefined;
}
