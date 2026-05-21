import { MissingExternal } from "missing-framework";
import { AdapterUser } from "../adapters";

export type AuthorizeTypes =
  | "oauth"
  | "email"
  | "magiclink"
  | "credentials"
  | "passkey";

export type Body = Record<string, any>;

export interface Provider {
  name: string;
  type: AuthorizeTypes;
  authorize: (body: Body) => Promise<AdapterUser | null>;
  callback?: (req: MissingExternal) => Promise<AdapterUser | null>;
}

export interface ChallengeStore {
  get: (userId: string) => Promise<string | null>;
}

export interface PasskeyProviderParams {
  store?: ChallengeStore;
}
