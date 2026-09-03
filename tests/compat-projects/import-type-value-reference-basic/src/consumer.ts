import type { greet, registry } from "./lib";
import type * as Lib from "./lib";

export type GreetFn = typeof greet;
export type RegistryShape = typeof registry;
export type NamespacedFn = typeof Lib.greet;
export type NamespacedOptions = Lib.Options;

declare const fn: GreetFn;
export const called: string = fn("ok");

export const asValue = greet;
