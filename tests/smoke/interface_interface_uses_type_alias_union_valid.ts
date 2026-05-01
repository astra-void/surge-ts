type MaybeName = string | undefined;

interface User {
  name?: MaybeName;
}

function f(user: User): MaybeName {
  return user.name;
}
