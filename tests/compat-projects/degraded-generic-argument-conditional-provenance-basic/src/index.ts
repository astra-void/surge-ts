import { createNext } from './alias';
import type { RouterBase } from './types';

const client = createNext<RouterBase>();

export function run(): string {
  client.withTRPC({ pathname: '/' });
  return client.router99();
}
