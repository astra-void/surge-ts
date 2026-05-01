function greet(user: { name: string }): string {
  return user.name;
}

greet({ name: "Ada", age: 36 });
