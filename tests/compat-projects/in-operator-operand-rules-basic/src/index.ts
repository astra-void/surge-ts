declare const count: number;
declare const text: string;
declare const flag: boolean;
declare const record: { a: number };
declare const maybeRecord: { a: number } | undefined;
declare const target: object;
declare const marker: "marker" & { __brand: "marker" };

const numberTarget = "a" in count;
const stringTarget = "a" in text;
const booleanKey = flag in record;
const objectKey = {} in record;
const possiblyUndefined = "a" in maybeRecord;

const stringKey = "a" in record;
const numberKey = 1 in target;
const brandedKey = marker in target;
