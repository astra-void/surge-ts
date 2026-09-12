interface Box {
  value: number;
  inner?: { deep: string };
  list: string[];
  run(): void;
}

declare const box: Box | undefined;
declare const nested: { box?: Box };
declare function lookup(): Box | undefined;
declare function nothing(): void;

export const named = box.value;
export const dotted = nested.box.value;
export const call = lookup().value;
export const methodOnUndefined = box.run();
export const elementOnUndefined = box.list[0];
export const bracketed = nested['box'].value;
export const voidResult = nothing().toString();

export function narrowedIsFine(b: Box | undefined): number {
  if (!b) {
    return 0;
  }
  return b.value + (b.inner?.deep.length ?? 0);
}

export function optionalChainContinues(b: Box | undefined): number {
  const viaChain = b?.inner.deep;
  const guarded = b?.inner?.deep ? b.inner.deep.length : 0;
  return (viaChain?.length ?? 0) + guarded;
}

export function assertionIsFine(b: Box | undefined): string {
  return b!.list.join(',');
}

export function ternaryIsFine(b: Box | undefined): number {
  return b ? b.value : 0;
}

export function andIsFine(b: Box | undefined): number {
  return (b && b.value) ?? 0;
}
