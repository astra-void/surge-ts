// A numeric key is converted to a string, so a string index signature answers it.
declare const rec: Record<string, boolean>;
declare const written: { [key: string]: boolean };
export const a: boolean = rec[1];
export const b: boolean = written[1];
