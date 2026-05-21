import { User } from "./user";

export interface AdapterUser extends User {
  role?: string;
}
