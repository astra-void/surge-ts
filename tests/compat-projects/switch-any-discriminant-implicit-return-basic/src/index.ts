declare const a: any;
export function onAny() {
  switch (a.status) {
    case 'idle':
      return 'a';
  }
}

declare const u: unknown;
export function onTypeofUnknown() {
  switch (typeof u) {
    case 'string':
      return 1;
  }
}

declare const s: 'x' | 'y';
export function onCoveredUnion() {
  switch (s) {
    case 'x':
      return 1;
    case 'y':
      return 2;
  }
}

declare const n: 'x' | 'y' | null;
export function onUncoveredNull() {
  switch (n) {
    case 'x':
      return 1;
    case 'y':
      return 2;
  }
}
