declare let numbers: { [x: string]: number };
let optionalMismatch: { a?: string } = numbers;
let optionalMatch: { a?: number } = numbers;
let intersected: { [x: string]: number } & { a?: string } = numbers;
let required: { a: number } = numbers;

interface ParsedUrlQuery { [key: string]: string | string[] | undefined }
declare let query: ParsedUrlQuery;
let nextQuery: ParsedUrlQuery & { amp?: '1' } = query;
export {};
