export interface ClientOptions {
  baseUrl?: string;
  throwOnError?: boolean;
}

// `T['baseUrl']` re-validates once per substitution. When the argument does not
// carry the key, tsc's instantiation path has no access node and yields its
// unknown type silently — the error would describe the instantiation, not
// anything written here.
export interface Config<T extends ClientOptions = ClientOptions> {
  baseUrl?: T['baseUrl'];
}

export declare const missingKey: Config<{ throwOnError: false }>;
export declare const presentKey: Config<{ baseUrl: 'https://example.com' }>;

// A key the argument does have still resolves.
export const used: string | undefined = presentKey.baseUrl;
