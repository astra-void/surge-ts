interface Passkey {
  id: string;
}

interface Adapter {
  createPasskey?: (userId: string) => Promise<Passkey>;
}

function mapPasskey(value: any): Passkey {
  return { id: String(value.id) };
}

export function createAdapter(db: any): Adapter {
  return {
    createPasskey: async (userId) => {
      const passkey = await db.create(userId);
      return mapPasskey(passkey);
    },
  };
}

export function wrongAdapter(db: any): Adapter {
  return {
    createPasskey: async (userId) => {
      return 123;
    },
  };
}
