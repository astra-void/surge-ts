declare let none: () => number;
declare let one: (x: number) => number;
declare let optional: (x?: number) => number;
declare let nullable: (x: number | undefined) => number;
declare let tail: (x: number, y?: number) => number;

export function relations(): void {
  none = one;
  none = optional;
  none = tail;
  optional = one;
  optional = nullable;
  one = optional;
}

type Handler = (data: string, options?: { strict: boolean }) => void;

export function contextual(): void {
  const plain: Handler = (data, options) => {
    const strict: boolean = options.strict;
  };
  const defaulted: Handler = (data, options = { strict: true }) => {
    const strict: boolean = options.strict;
  };
  [1, 2].forEach((value, index) => index - 1);
  [1, 2].reduce((sum: number, value: number, index: number) => sum + index, 0);
}

enum Level {
  Low,
  High,
}
declare let missing: typeof undefined;

export function writes(n: number): void {
  Level.Low = n;
  missing = 1;
}
