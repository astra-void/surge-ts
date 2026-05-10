import { AdapterUser } from "../../adapters";

export interface User {
  id: string;
  email?: string;
}

export interface Session {
  data: {
    user: User | AdapterUser | null;
  };
}
