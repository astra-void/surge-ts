type NotAString = string extends `a${infer T}` ? T : 0;
type HeadTail = "abc" extends `${infer H}${infer R}` ? [H, R] : 0;
type NoPrefix = "abc" extends `x${infer R}` ? R : 0;
type AsNumber = "100" extends `${infer T extends number}` ? T : 0;
type NotRoundTrip = "1.0" extends `${infer T extends number}` ? T : 0;
type NotNumeric = "abc" extends `${infer T extends number}` ? T : 0;
type AsBoolean = "true" extends `${infer T extends boolean}` ? T : 0;
type Split = "a.b.c" extends `${infer H}.${infer R}` ? [H, R] : 0;
type Empty = "" extends `${infer H}${infer R}` ? [H, R] : 0;
type Prefix = "100" extends `${infer T extends number}${string}` ? T : never;

type Split2<S extends string> = S extends `${infer Head}/${infer Rest}` ? [Head, Rest] : [S];

export const ok: [NotAString, HeadTail, NoPrefix, AsNumber, NotRoundTrip, NotNumeric, AsBoolean, Split, Empty] =
    [0, ["a", "bc"], 0, 100, 1, 0, true, ["a", "b.c"], 0];
export const path: Split2<"a/b/c"> = ["a", "b/c"];
export const wrongNumber: AsNumber = 1;
export const firstDigit: Prefix = 100;
export const wrongPath: Split2<"a/b"> = ["a", "c"];
