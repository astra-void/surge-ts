interface AdapterUser {
  id: string;
  email?: string;
}

interface Adapter {
  createUser?: (email: string, hashedPassword: string, username?: string) => Promise<AdapterUser>;
  getUserByEmail?: (email: string) => Promise<AdapterUser | null>;
}

interface Provider {
  authorize: (body: { [key: string]: any }) => Promise<AdapterUser | null>;
}

function mapUser(user: AdapterUser): AdapterUser {
  return {
    id: user.id,
    email: user.email,
  };
}

export function adapterFactory(db: any): Adapter {
  return {
    createUser: async (email, hashedPassword, username) => {
      const user = await db.create({ email, hashedPassword, username });
      return mapUser(user);
    },
    getUserByEmail: async (email) => {
      const user = await db.getByEmail(email);
      return user ? mapUser(user) : null;
    },
  };
}

export function providerFactory(adapter: Adapter): Provider {
  return {
    authorize: async (body) => {
      const email = body.email;
      if (!email) {
        return null;
      }

      const user = await adapter.getUserByEmail?.(email);
      return user ?? null;
    },
  };
}

export function badProvider(): Provider {
  return {
    authorize: async (_body) => {
      return { nope: true };
    },
  };
}
