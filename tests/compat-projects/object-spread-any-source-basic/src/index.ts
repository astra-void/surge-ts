interface RestoreOptions {
  client: string;
  persister: { restore(): void };
  maxAge?: number;
}

declare function restore(options: RestoreOptions): void;
declare const loose: any;
declare const client: string;

// `{ ...any, k: v }` is `any`: the spread may carry every required member.
export function viaAnySpread() {
  const options = { ...loose, client };
  restore(options);
  return options.persister;
}

// A closed object spread keeps its shape: a wrong member is still reported.
export function viaObjectSpread(partial: { persister: number }) {
  const options = { ...partial, client };
  restore(options);
}

// Not a suppression: an ordinary mismatch is still reported.
declare const count: number;
export const bad: string = count;
