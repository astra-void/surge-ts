import type { Adapter, AdapterUser } from "../adapters/types";

export interface ChallengeStore {
  get(id: string): Promise<string | null>;
}

export interface CredentialsProviderConfig<
  Body = { email?: string; password?: string }
> {
  authorize: (body: Body) => Promise<AdapterUser | null>;
  callback?: (req: unknown) => Promise<AdapterUser | null>;
  store?: ChallengeStore;
  adapter?: Adapter;
}
