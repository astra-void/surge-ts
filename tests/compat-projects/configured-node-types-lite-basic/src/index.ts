export interface Passkey {
  publicKey: Buffer;
  raw: Buffer<ArrayBuffer>;
}

export function read(): string | undefined {
  const secret = process.env.AUTHKIT_SECRET;
  const value = Buffer.from("abc", "base64url");
  return secret ?? value.toString("base64url");
}
