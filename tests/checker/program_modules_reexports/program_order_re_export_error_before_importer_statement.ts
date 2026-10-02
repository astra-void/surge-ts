// @surge-compare: order
// @filename: user.ts
export const version: number = 1;
// @filename: index.ts
export { Foo } from "./user";
// @filename: app.ts
let value: string = 123;
