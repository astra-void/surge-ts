interface User {
  name?: string;
}

function f(user: User): string | undefined {
  return user.name;
}
