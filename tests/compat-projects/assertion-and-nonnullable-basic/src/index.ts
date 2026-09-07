declare function assertIsObject(
  value: unknown,
): asserts value is Record<string, unknown>;
declare function assertDefined(value: unknown): asserts value;

export function readMembers(message: unknown): unknown[] {
  assertIsObject(message);
  return [message['id'], message['method']];
}

export function truthyAssertion(name: string | undefined): number {
  assertDefined(name);
  return name.length;
}

interface Options {
  lazy?: { enabled: boolean; closeMs: number };
}
type LazyOptions = Required<NonNullable<Options['lazy']>>;
declare const lazyDefaults: LazyOptions;
declare function afterMs(callback: () => void, ms: number): void;

export function schedule(opts: Options): boolean {
  const merged = { ...lazyDefaults, ...opts.lazy };
  afterMs(() => {}, merged.closeMs);
  const enabled: boolean = merged.enabled;
  return enabled;
}

export function nonNullableDropsUndefined(value: NonNullable<string | undefined>): number {
  return value.length;
}

export function withoutTheAssertion(message: unknown): unknown {
  return message['id'];
}
