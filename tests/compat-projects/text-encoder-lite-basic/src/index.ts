export function encodeToken(input: string): Uint8Array {
  return new TextEncoder().encode(input);
}
