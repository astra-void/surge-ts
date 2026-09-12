interface Bag {
  patterns?: Set<string>;
  allOf?: string[];
  count?: number;
}

export function nullish(bag: Bag): number {
  bag.patterns ??= new Set();
  bag.patterns.add('p');
  return bag.patterns.size;
}

export function coalesceAssign(bag: Bag): number {
  bag.allOf = bag.allOf ?? [];
  bag.allOf.push('x');
  return bag.allOf.length;
}

export function orAssign(bag: Bag): number {
  bag.count ||= 1;
  return bag.count.toFixed().length;
}

export function local(): number {
  let cached: string[] | null = null;
  cached ??= [];
  return cached.length;
}

export function unassignedStillReports(bag: Bag): number {
  return bag.count.toFixed().length;
}
