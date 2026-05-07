async function digest(value: string): Promise<Uint8Array> {
  return new Uint8Array(value.length);
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((value: number) => value.toString(16).padStart(2, "0"))
    .join("");
}

export async function createToken(secret: string): Promise<string> {
  const tokenArray = new Uint8Array(8);
  const token = toHex(tokenArray);
  const data = new TextEncoder().encode(`${token}:${Date.now()}`);
  const hmacDigest = await digest(secret);
  const hmacHex = toHex(hmacDigest);
  return `${token}:${hmacHex}:${data.length}`;
}
