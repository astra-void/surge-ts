interface ClientState {
  mutations: string[];
  queries: { queryKey: unknown[] }[];
}

interface PersistedClient {
  timestamp: number;
  clientState: ClientState;
}

type Promisable<T> = T | Promise<T>;

interface Persister {
  restoreClient: () => Promisable<PersistedClient | undefined>;
}

declare const persister: Persister;

export async function restoreAndRead(): Promise<number | undefined> {
  const restored = await persister.restoreClient();

  // `restored?.clientState.queries` continues the chain: the `undefined` the
  // chain contributes must not read as a possibly-undefined `clientState`.
  const count = restored?.clientState.queries.length;
  const found = restored?.clientState.queries.find((q) => q.queryKey[0] === 'A');

  // Truthiness narrowing removes the awaited `undefined` too.
  if (restored) {
    const stamp: number = restored.timestamp;
    void stamp;
  }

  void found;
  return count;
}

export async function stillReports(): Promise<void> {
  const restored = await persister.restoreClient();
  const bad: number = restored;
  void bad;
}
