import Emitter from "emitkit";

type Events = { a: string[] };

export class Mine extends Emitter<Events> {
  extra(): string {
    return "x";
  }
}

declare const e: Emitter<Events>;
export const data = e.data;
export const bad: number = e.data;
