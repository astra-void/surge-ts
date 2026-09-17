const named: ReadonlyArray<number> = [1];
named[0] = 3;

declare const declared: ReadonlyArray<string | number>;
declared[1] = "a";

const spelled: readonly number[] = [1];
spelled[0] = 2;

function parameter(values: ReadonlyArray<string>) {
  values[1] = "a";
}

const tuple: readonly [number, string] = [1, "a"];
tuple[0] = 2;

const mutable: number[] = [1];
mutable[0] = 2;
