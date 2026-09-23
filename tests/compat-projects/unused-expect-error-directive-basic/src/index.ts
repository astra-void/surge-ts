export const a: number = 1;
// @ts-expect-error
const b: string = 1;
// @ts-expect-error
const c: string = "ok";
// @ts-ignore
const d: string = "ok";
/* @ts-expect-error */
const e: string = "ok";
/*
 * multi
 @ts-expect-error */
const f: string = "ok";
// @ts-expect-error
// eslint-disable-next-line
const g: string = 1;
// @ts-expect-error
// eslint-disable-next-line
const h: string = "ok";
// @ts-expect-error
// @ts-ignore
const i: string = 1;
// not a directive: @ts-expect-error
const j: string = "ok";
const k: string = "ok"; // @ts-expect-error
const l: string = "ok";
// @ts-expect-error
