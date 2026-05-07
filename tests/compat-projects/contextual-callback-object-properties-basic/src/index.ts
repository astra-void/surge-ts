interface AdapterUser {
  id: string;
  email: string;
  hashedPassword?: string;
}

interface Adapter {
  getUserByEmail?(email: string): Promise<AdapterUser | null>;
  createUser?(email: string, hashedPassword: string): Promise<AdapterUser | null>;
}

interface CredentialsProviderConfig<
  Body = { email?: string; password?: string }
> {
  authorize: (body: Body) => Promise<AdapterUser | null>;
}

function hashPassword(
  password: string,
  algorithm: "argon2" | "scrypt" = "argon2",
  salt = ""
) {
  return password + algorithm + salt;
}

function Credentials(config: { adapter: Adapter; algorithm?: "argon2" | "scrypt" }) {
  return {
    authorize: async (body: { email?: string; password?: string }) => {
      const { email, password } = body;
      if (!email || !password) return null;

      const existingUser = await config.adapter.getUserByEmail?.(email);
      if (existingUser) return null;

      const hashedPassword = await hashPassword(password, config.algorithm ?? "argon2");
      const user = await config.adapter.createUser?.(email, hashedPassword);
      if (!user) return null;

      return user;
    },
  } satisfies CredentialsProviderConfig;
}
