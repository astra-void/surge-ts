function take(user: User): string {
  return user.name;
}

take({ name: "Ada" });

interface User {
  name: string;
}
