type Code = { kind: "code"; grade: number };
type NameOnly = { kind: "name-only"; name: string };
type Row = { identity: Code | NameOnly };

declare function isCode(row: Row): row is Row & { identity: Code };
declare function isString(value: unknown): value is string;

declare const rows: Row[];
declare const mixed: (string | number)[];

export const grades = rows.filter(isCode).map((row) => row.identity.grade);
export const strings: string[] = mixed.filter(isString);

declare const narrowed: Row & { identity: Code };
export const one: number = narrowed.identity.grade;

export const kept: Row[] = rows.filter((row) => row.identity.kind === "code");
export const wrong: number[] = mixed.filter(isString);
