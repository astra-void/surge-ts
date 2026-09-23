import { Factory, createFactory } from "./factory.js";

export const made: Factory = Factory.create();
export const wrong: string = Factory.create(1);
export const viaExport: number = createFactory();
export const counted: number = Factory.withDefault();
export const badCount = Factory.withDefault("three");
export const list: string[] = Factory.generic(1);
export const plain: number = Factory.plain("a");
export const built: number = new Factory().build("a");
