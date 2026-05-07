export async function hashPassword(
  password: string,
  algorithm: "argon2" | "scrypt" = "argon2",
  salt = ""
): Promise<string | null> {
  return password + algorithm + salt;
}

export async function verifyPassword(
  password: string,
  hash: string,
  algorithm: "argon2" | "scrypt" = "argon2",
  salt = ""
): Promise<boolean | null> {
  return password.length > 0 && hash.length > 0 && salt.length >= 0;
}

async function run(config: { algorithm?: "argon2" | "scrypt" }) {
  await hashPassword("pw");
  await hashPassword("pw", config.algorithm ?? "argon2");
  await verifyPassword("pw", "hash");
  await verifyPassword("pw", "hash", config.algorithm ?? "argon2");
}
