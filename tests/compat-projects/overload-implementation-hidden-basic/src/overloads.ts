export function parse(input: string): string;
export function parse(input: number): number;
export function parse(input: any): any {
  return input;
}

export function pair(a: string, b: number): void;
export function pair(a: number, b: string): void;
export function pair(a: any, b: any): void {
  void a;
  void b;
}

export function arity(only: string): void;
export function arity(first: number, second: number): void;
export function arity(first: any, second?: any): void {
  void first;
  void second;
}
