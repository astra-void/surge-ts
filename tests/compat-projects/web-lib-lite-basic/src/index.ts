type Transport = AuthenticatorTransport;

export async function call(url: string): Promise<number> {
  const res = await fetch(url);
  return res.status;
}

export type Bytes = ArrayBuffer;
