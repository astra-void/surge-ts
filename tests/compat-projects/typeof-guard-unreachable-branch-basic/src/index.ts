export interface Config {
  url: string;
  authToken?: string;
}

declare function createClient(config: Config): { ok: true };

export function fromEitherShape(connection: Config | undefined): { ok: true } {
  return typeof connection === 'string'
    ? createClient({ url: connection })
    : createClient(connection!);
}

export function theStringBranchIsUnreachable(value: Config | undefined): string {
  if (typeof value === 'string') {
    return value;
  }
  return 'no';
}

export function aUnionWithAStringStillNarrows(value: Config | string | undefined): string {
  if (typeof value === 'string') {
    return value;
  }
  return 'no';
}

export function theOtherBranchIsUnreachableToo(value: string | 'literal'): number {
  if (typeof value === 'string') {
    return 1;
  }
  return value;
}

export function theSurvivingMembersAreStillChecked(value: Config | string | undefined): string {
  if (typeof value === 'string') {
    return value;
  }
  return value.url;
}
