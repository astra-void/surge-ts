export async function getSecureRandomBytes(len = 32): Promise<Uint8Array> {
  if (typeof globalThis !== "undefined" && globalThis.crypto) {
    return globalThis.crypto.getRandomValues(new Uint8Array(len));
  }

  try {
    const crypto = await import("crypto");
    return crypto.randomBytes(len);
  } catch {
    throw new Error("No crypto");
  }
}
