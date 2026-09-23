import greeting, * as module from "./m";
export const text: string = greeting;
export const value: number = module.other;
export const same: string = module.default;
export const missing = module.absent;
