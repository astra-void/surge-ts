declare function elementAt<K extends [string, number?]>(key: K, at: K[1]): void;

export function callsWithATupleOfOne(): void {
  elementAt([''], undefined);
}

export const wrapped = <K extends [string, Record<string, unknown>?]>(
  key: K,
  fetcher: (options: K[1]) => void,
) => [key, fetcher];

export const fromTheWrapper = wrapped([''], () => {});
