import { User } from "./user";
import { Passkey } from "./passkey";

export interface AdapterUser extends User {
  passkeys?: Passkey[];
}

export interface Adapter {
  createUser?: () => Promise<AdapterUser>;
  getPasskey?: () => Promise<Passkey[] | null>;
}

const ok: AdapterUser = { id: "u1", passkeys: [{ id: "p1" }] };
const missing: AdapterUser = { passkeys: [] };
