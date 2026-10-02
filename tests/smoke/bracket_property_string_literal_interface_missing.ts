// @noImplicitAny: true
interface User {
  id: string;
}

function getName(user: User): string {
  return user["name"];
}
