interface User {
  name?: string | undefined;
}

function f(user: User): string | undefined {
  return user.name;
}
