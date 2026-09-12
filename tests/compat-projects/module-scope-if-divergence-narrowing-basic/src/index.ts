declare function isCancel(value: unknown): value is symbol;
declare function die(): never;
declare function log(message: string): void;
declare const transforms: symbol | string[];
declare const other: symbol | string[];
declare const kept: symbol | string[];
declare const wide: symbol | string[];

if (isCancel(transforms)) die();
export const sorted: string[] = transforms.sort();

if (isCancel(other)) {
  throw new Error("cancelled");
}
export const second: number = other.length;

if (!isCancel(kept)) {
  die();
}
export const symbolic: symbol = kept;

if (isCancel(wide)) {
  log("no exit");
}
export const stillWide = wide.sort();
