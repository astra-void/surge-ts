import { greet, version } from "flat-types";

export const message: string = greet(version);
export const wrong: number = greet("x");
