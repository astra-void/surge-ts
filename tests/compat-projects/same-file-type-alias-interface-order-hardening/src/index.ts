import { MissingExternal } from "missing-framework";

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
  config?: Record<string, any>;
  authorize: (body: Body) => Promise<AdapterUser | null>;
  callback?: (req: MissingExternal) => Promise<AdapterUser | null>;
}

export interface AdapterUser {
  id: string;
}

export interface ChallengeStore {
  get: (userId: string) => Promise<string | null>;
  set: (userId: string, challenge: string) => Promise<void>;
  delete: (userId: string) => Promise<void>;
}

export interface PasskeyProviderParams {
  store?: ChallengeStore;
}
