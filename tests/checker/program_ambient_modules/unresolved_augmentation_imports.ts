// @filename: dep.ts
export class B { value!: number }
// @filename: index.ts
export {};
declare module "./missing" {
    import { B } from "./dep";
    interface Missing { value: B }
}
