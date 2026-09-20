declare const table: Record<string, number>;
declare function key(): string;
declare function step(): number;

export const named = { [table.length ? "a" : "b"]: 1 };
export const computed = { [key()]: missingComputedValue };
export const templated = { [`k${missingTemplatePart}`]: 1 };

export function loops(): number {
  let total = 0;
  for (let index = 0; index < 4; index += step(), missingUpdate()) {
    total += index;
  }
  step(), missingSequence();
  const last = (step(), missingSequenceValue);
  return total + last;
}

export class Holder {
  unresolved: MissingAnnotation;
  assigned: string;

  constructor() {
    this.assigned = "";
  }
}
