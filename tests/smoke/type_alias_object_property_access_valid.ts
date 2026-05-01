type User = { name?: string };

function getName(user: User): string | undefined {
  return user.name;
}
