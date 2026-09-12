interface Persisted { clientState: { queries: string[] } }
interface Opts { flag?: boolean; mode: 'a' | 'b' }
declare function save(client: Persisted): Promise<Error | undefined>;
declare function findQuery(): { isFetched(): boolean; reset(): void } | undefined;
declare function run(callback: (persisted: Persisted) => Promise<void>): void;

export async function initializerNarrows(persisted: Persisted) {
  let client: Persisted | undefined = persisted;
  await save(client);
}

run(async (persisted) => {
  let client: Persisted | undefined = persisted;
  await save(client);
});

export function assignmentUsesDeclaredType(options: Opts) {
  if (options.flag === undefined) {
    options.flag = options.mode === 'a';
  }
}

export function aliasInsideChain(mode: string) {
  const query = findQuery();
  const isRefetch = query !== undefined && query.isFetched();
  if (isRefetch && mode === 'reset') {
    query.reset();
  }
}

export function negatedAliasInsideChain(mode: string) {
  const query = findQuery();
  const missing = query === undefined;
  if (!missing && mode === 'reset') {
    query.reset();
  }
}

export function stillReportsWithoutTheGuard(mode: string) {
  const query = findQuery();
  if (mode === 'reset') {
    query.reset();
  }
}

export function wrongAssignment(options: Opts) {
  if (options.flag === undefined) {
    options.flag = options.mode;
  }
}
