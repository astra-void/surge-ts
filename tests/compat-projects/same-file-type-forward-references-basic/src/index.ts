interface User {
  id: string;
}

export interface AdapterUser extends User {
  passkeys?: Passkey[];
}

export interface Passkey {
  id: string;
  owner?: AdapterUser;
}

export interface Adapter {
  createUser?: (email: string) => Promise<AdapterUser>;
  getPasskey?: (userId: string) => Promise<Passkey[] | null>;
  updatePasskey?: (passkeyId: string, data: Partial<Passkey>) => Promise<Passkey>;
  missing?: MissingLater;
}
