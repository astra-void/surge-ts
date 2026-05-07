import type { Adapter, AdapterUser } from "../adapters/types";

export interface ChallengeStore {
  get(id: string): Promise<string | null>;
}

export interface ProviderConfig {
  adapter: Adapter;
  callback?: (req: unknown) => Promise<AdapterUser | null>;
  passkeys?: Passkey[];
  store?: ChallengeStore;
}
