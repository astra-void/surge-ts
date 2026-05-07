export function session(
  user: { id: string } | null,
  status: "loading" | "authenticated"
) {
  if (!user) {
    return { data: null, status };
  }

  return { data: { user, status } };
}
