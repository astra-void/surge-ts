type Perf = { measure: number };
declare function isReady(): boolean;

export function falsyBranch(
  text: string | Perf,
  count: number | Perf,
  flag: boolean | Perf,
  maybe: Perf | undefined,
  literals: "x" | "" | Perf,
  digits: 1 | 0 | Perf,
  nullable: Perf | null,
  list: string[] | undefined,
  callback: (() => void) | undefined,
  big: bigint | Perf,
) {
  if (!text) { const t: never = text; }
  if (!count) { const t: never = count; }
  if (!flag) { const t: never = flag; }
  if (!maybe) { const t: never = maybe; }
  if (!literals) { const t: never = literals; }
  if (!digits) { const t: never = digits; }
  if (!nullable) { const t: never = nullable; }
  if (!list) { const t: never = list; }
  if (!callback) { const t: never = callback; }
  if (!big) { const t: never = big; }
}

export function afterEarlyExit(perf: Perf | undefined, other: false | Perf) {
  if (perf) {
    return perf.measure;
  }
  const gone: string = perf;
  const viaElse = other ? other.measure : other;
  const asNumber: number = viaElse;
  return [gone, asNumber];
}

export function alwaysTruthy(perf: Perf) {
  if (!perf) {
    return perf.measure;
  }
  return isReady || isReady();
}
