export function wrongReturn(x: string): number;
export function wrongReturn(x: number): string {
  return String(x);
}

export function voidOverload(x: string): void;
export function voidOverload(x: string | number): number {
  return 1;
}

export function tooManyRequired(x: string): void;
export function tooManyRequired(x: string, y: number) {
  void y;
}

export function wrongParameter(x: boolean): void;
export function wrongParameter(x: string) {
  void x;
}

export function optionalAgainstRequired(x?: number): void;
export function optionalAgainstRequired(x: number) {
  void x;
}

export function narrowerImplementation(x: "a" | "b"): void;
export function narrowerImplementation(x: "a") {
  void x;
}

export function spread(...values: number[]): void;
export function spread(...values: unknown[]) {
  void values;
}

export function tupleRest(name: string, value: number): void;
export function tupleRest(...args: [string, number?]) {
  void args;
}
