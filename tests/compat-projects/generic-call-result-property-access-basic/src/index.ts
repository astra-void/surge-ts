interface Response<T> {
  status: number;
  data: T;
}

declare function authRequest<T>(
  method: string,
  url: string,
  body?: object
): Promise<Response<T> | null>;

export async function login(body: object): Promise<boolean | undefined> {
  const verification = (
    await authRequest<{ success: boolean }>("POST", "/api", body)
  )?.data.success;
  return verification;
}

export async function anyCall(body: object) {
  const value = (await authRequest<any>("POST", "/api", body))?.data.success;
  return value;
}
