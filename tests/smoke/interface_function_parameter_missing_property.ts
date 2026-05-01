interface User {
  name: string;
}

function take(user: User): string {
  return user.name;
}

take({});
