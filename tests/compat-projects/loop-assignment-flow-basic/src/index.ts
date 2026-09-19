declare const cond: boolean;

interface DescriptionMap {
  properties: Map<string, string>;
}

export function reduced(): void {
  let d: DescriptionMap | null = null;
  d ??= { properties: new Map() };
  const s: string = d;
}

export function maxOf(values: number[]): void {
  let max: number | null = null;
  for (const v of values) {
    if (max === null || v > max) {
      max = v;
    }
  }
  const s: string = max;
}

export function backEdge(): void {
  let x: string | number | boolean = "";
  while (cond) {
    const b: boolean = x;
    x = 1;
  }
  const after: boolean = x;
}

export function exhaustedNull(value: null): void {
  if (value !== null) {
    const n: number = value;
  }
}
