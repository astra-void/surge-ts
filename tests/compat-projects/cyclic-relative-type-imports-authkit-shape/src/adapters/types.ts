import { User } from "../react/hooks/types";

export interface AdapterUser extends User {
  role?: "admin" | "member";
  passkeys?: Passkey[];
}

export interface Passkey {
  id: string;
  userId: string;
  counter: number;
}

export interface Adapter {
  getUser?: (id: string) => Promise<AdapterUser | null>;
  getPasskeys?: (userId: string) => Promise<Passkey[] | null>;
}
