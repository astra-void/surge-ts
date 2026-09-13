export function compact(values: (string | undefined)[]): unknown {
  return values.filter((c): c is Exclude<typeof c, undefined> => c !== undefined);
}

export function namedTargetStillNarrows(values: (string | number)[]): unknown {
  const strings: string[] = values.filter((value): value is string => typeof value === 'string');
  return strings;
}

export function theNarrowingIsStillChecked(values: (string | number)[]): unknown {
  const numbers: number[] = values.filter((value): value is string => typeof value === 'string');
  return numbers;
}
