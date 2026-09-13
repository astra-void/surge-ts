import { queryKey, sleep } from '@acme/query-utils';

const key = queryKey();

export const first: string | undefined = key[0];
export const viaCallback = [['query_1']].find((entry) => entry[0] === key[0]);
export const length: number = key.length;

export async function later(): Promise<string | undefined> {
  await sleep(1);
  const inner = queryKey();
  return inner[0];
}
