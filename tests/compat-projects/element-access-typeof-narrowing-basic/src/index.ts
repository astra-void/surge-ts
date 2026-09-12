export function inAStatement(args: [first?: string | number, second?: unknown]) {
  if (typeof args[0] === 'string') {
    const first: string = args[0];
    return first;
  }
  return '';
}

export function inATernary(args: [first?: string | number, second?: unknown]) {
  const first: string | undefined =
    typeof args[0] === 'string' ? args[0] : undefined;
  return first;
}

export function theUnknownKeywordTakesTheTag(args: [first?: unknown | string]) {
  const first: string | undefined =
    typeof args[0] === 'string' ? args[0] : undefined;
  return first;
}

export function theNarrowedReadIsStillTheTag(
  args: [first?: string | number, second?: unknown],
) {
  if (typeof args[0] === 'string') {
    const wrong: number = args[0];
    return wrong;
  }
  return 0;
}
