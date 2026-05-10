export interface Session {
  data: {
    user: { id: string } | null;
  };
}

export async function authRequest<T>(
  method: "POST" | "GET" | "PUT" | "DELETE",
  url: string,
  body?: object
): Promise<{ data: T; status: number } | null> {
  return null as any;
}

export async function getSession(): Promise<{ id: string } | null> {
  const res = await authRequest<Session>("GET", "/api/auth/session");

  if (res?.status !== 200 || !res.data) return null;

  const data = res.data.data;

  if (data.user) {
    return data.user;
  }

  return null;
}
