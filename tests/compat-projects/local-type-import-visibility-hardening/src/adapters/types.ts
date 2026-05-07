export interface AdapterUser {
  id: string;
}

export interface Passkey {
  id: string;
}

export interface Adapter {
  getUser(id: string): Promise<AdapterUser | null>;
}
