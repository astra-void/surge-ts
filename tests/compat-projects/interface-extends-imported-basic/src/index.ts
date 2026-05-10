import { User } from "./base";

export interface AdapterUser extends User {
  role?: string;
}

const ok: AdapterUser = { id: "u1", role: "admin" };
const missing: AdapterUser = { role: "admin" };
const wrong: AdapterUser = { id: 123 };
