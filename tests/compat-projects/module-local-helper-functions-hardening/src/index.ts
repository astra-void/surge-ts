function bytesToBase32(bytes: string): string {
  return bytes.replace(/=+$/, "").toUpperCase();
}

function base32ToBytes(base32: string): string {
  const cleaned = base32.replace(/=+$/, "").toUpperCase();
  return cleaned.toLowerCase();
}

function intToUint8Array(num: number): string {
  const arr = num.toString().padStart(8, "0");
  return arr;
}

export function generateTOTP(secret: string, windowRange = 1): string {
  const key = base32ToBytes(secret);
  const counter = intToUint8Array(windowRange);
  const otp = bytesToBase32(key + counter);
  return otp.toString().padStart(6, "0");
}
