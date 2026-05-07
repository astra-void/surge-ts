export function verify(cookieToken?: string, headerToken?: string): boolean {
  if (!cookieToken || !headerToken || cookieToken !== headerToken) return false;

  const parts = cookieToken.split(":");
  if (parts.length !== 3) return false;

  const [token, timestampStr, hmacHex] = parts;
  const timestamp = Number(timestampStr);
  if (isNaN(timestamp)) return false;

  const expectedHmacHex = hmacHex.toUpperCase();
  return token.length > 0 && expectedHmacHex.length > 0;
}

export function objectInput(input: { email?: string; password?: string }) {
  const { email, password } = input;
  if (!email || !password) return null;
  return { email, password };
}
