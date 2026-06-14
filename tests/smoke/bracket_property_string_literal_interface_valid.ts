interface User {
  id: string;
}

function getId(user: User): string {
  return user["id"];
}
