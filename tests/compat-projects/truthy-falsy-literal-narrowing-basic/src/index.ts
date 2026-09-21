type Perf = { mark(): void; measure: number };
declare const inBrowser: boolean;
declare const source: Perf | undefined;

export function falsyLiterals(
  flagged: false | { go(): void },
  text: "" | { n: number },
  count: 0 | { n: number },
) {
  if (flagged) {
    flagged.go();
    const wrong: string = flagged;
    return wrong;
  }
  const viaAnd = text && text.n;
  const viaTernary = count ? count.n : 0;
  return [viaAnd, viaTernary];
}

export function aliasedInIf() {
  const perf = inBrowser && source;
  if (perf) {
    perf.mark();
    const wrong: string = perf;
    return wrong;
  }
  return 0;
}

export function aliasedInChain() {
  const perf = inBrowser && source;
  if (perf && perf.measure) {
    perf.mark();
  }
  return perf ? perf.measure : 0;
}

export function aliasedInWhile() {
  const perf = inBrowser && source;
  while (perf) {
    return perf.measure;
  }
  return 0;
}

export function classicAlias(value: string | number) {
  const isText = typeof value === "string";
  if (isText) {
    const wrong: number = value;
    return wrong;
  }
  const rest: string = value;
  return rest;
}
