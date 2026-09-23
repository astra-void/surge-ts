import { Aliased, Nested } from "./c";
const v: number = Aliased;
const w: Nested.Widget = new Nested.Widget();
const wrong: string = w.size;
