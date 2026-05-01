type User = { name: string; age?: number };

function take(user: User): string {
  return user.name;
}

take({ name: "Ada" });
