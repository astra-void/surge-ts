function pick(x: number): number;
function pick(x: string): string;
function pick(x: number | string): number | string {
  return x;
}
