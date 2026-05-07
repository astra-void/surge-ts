export function generateRandomToken(length = 32): string {
  return String(length);
}

export async function hashPassword(
  password: string,
  algorithm: "argon2" | "scrypt" = "argon2",
  salt = ""
): Promise<string | null> {
  return salt || password || algorithm;
}

export function error(message = "Internal server error", status = 500) {
  return { message, status };
}

export function verifyTOTP(secret: string, windowRange = 1): boolean {
  return windowRange > 0 && secret.length > 0;
}

export function mustAnnotate(value) {
  return value;
}
