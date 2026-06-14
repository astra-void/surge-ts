function getId(user: { id: string } | undefined): string | undefined {
  return user?.["id"];
}
