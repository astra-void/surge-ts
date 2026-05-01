interface User {
  name?: string;
}

function f(user: User): string {
  return user.name;
}
