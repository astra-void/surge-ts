function take(user: { name: string; age?: number }): string {
  return user.name;
}

take({ age: 36 });
