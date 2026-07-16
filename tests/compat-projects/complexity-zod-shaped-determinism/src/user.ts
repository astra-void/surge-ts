import { string, number, Schema } from "./schema";

export const nameSchema: Schema<string> = string();
export const ageSchema: Schema<number> = number();

const name: string = nameSchema.parse("ada");
const age: number = ageSchema.parse(42);
const wrongName: number = nameSchema.parse("x");
const list: string[] = nameSchema.array().parse([]);
const opt: string | undefined = nameSchema.optional().parse(undefined);

export { name, age, wrongName, list, opt };
