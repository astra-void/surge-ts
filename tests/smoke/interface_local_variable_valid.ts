interface User {
  name: string;
}

function f(): string {
  let user: User = { name: "Ada" };
  return user.name;
}
