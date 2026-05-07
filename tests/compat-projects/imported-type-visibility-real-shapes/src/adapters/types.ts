export interface AdapterUser {
  id: string;
  email: string;
}

export interface Passkey {
  id: string;
}

export interface Adapter {
  getUser(id: string): Promise<AdapterUser | null>;
  getUserByEmail?(email: string): Promise<AdapterUser | null>;
  createUser?(email: string, hashedPassword: string): Promise<AdapterUser | null>;
}
