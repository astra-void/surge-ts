import { User } from "../react/hooks/types";

export interface AdapterUser extends User {
  role?: string;
  passkeys?: Passkey[];
  awaitingTotp?: boolean;
}

export interface Passkey {
  id: string;
  publicKey: Buffer;
  userId: string;
  webAuthnId: Buffer;
  counter: number;
  transports: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface Adapter {
  createUser?: (email: string, hashedPassword: string, username?: string) => Promise<AdapterUser>;
  getUser?: (id: string) => Promise<AdapterUser | null>;
  getPasskey?: (userId: string) => Promise<Passkey[] | null>;
}
