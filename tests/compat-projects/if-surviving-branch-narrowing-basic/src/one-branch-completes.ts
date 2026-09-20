declare function log(value: unknown): void;

export function elseExits(value: string | number): number {
  if (typeof value === "number") {
    log(value);
  } else {
    return 0;
  }
  return value;
}

export function elseAssigns(value: string | number): string {
  if (typeof value === "string") {
    return value;
  } else {
    value = "now a string";
  }
  return value;
}

export function elseContinues(value: string | number | boolean): void {
  if (typeof value === "string") {
    return;
  } else {
    log(value);
  }
  const rest: number | boolean = value;
  const wrong: string = value;
  log([rest, wrong]);
}

export function thenAssigns(value: string | number | undefined): void {
  if (value !== undefined) {
    value = 1;
  } else {
    throw new Error("missing");
  }
  const count: number = value;
  const wrong: string = value;
  log([count, wrong]);
}

export function bothComplete(value: string | number): void {
  if (typeof value === "string") {
    log(value);
  } else {
    log(value);
  }
  const asString: string = value;
  const asNumber: number = value;
  log([asString, asNumber]);
}

export function branchLocal(value: string | number): void {
  if (typeof value === "string") {
    return;
  } else {
    const inner: string | number = value;
    if (typeof inner === "number") log(inner);
  }
  const wrong: string = value;
  const right: number = value;
  log([wrong, right]);
}

export function reassignedAfter(value: string | number): void {
  if (typeof value === "string") {
    return;
  } else {
    log(value);
  }
  value = "text";
  const text: string = value;
  const wrong: number = value;
  log([text, wrong]);
}

export function loopBreak(items: (string | number)[]): void {
  for (const item of items) {
    if (typeof item === "string") {
      break;
    } else {
      log(item);
    }
    const count: number = item;
    const wrong: string = item;
    log([count, wrong]);
  }
}
