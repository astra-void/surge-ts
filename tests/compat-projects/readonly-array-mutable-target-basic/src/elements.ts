declare const open: readonly [string, number, ...boolean[]];
declare const pair: readonly [number, string];
declare const list: readonly number[];
declare const byName: ReadonlyArray<number>;

open[0] = "";
open[1] = 1;
open[2] = true;
open[0 + 1] = 1;
pair[0] = 1;
pair[0 + 1] = 1;
list[0] = 1;
byName[0] = 1;
delete open[0];
delete open[2];
delete open[0 + 1];
delete pair[0];
delete pair[1];
delete list[0];
delete list[0 + 1];
delete byName[0];

export {};
