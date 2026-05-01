type MaybeName = string | undefined;
type User = { name?: MaybeName };

function getName(user: User): string {
  return user.name;
}
