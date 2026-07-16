import { boolean } from "./schema";
import { nameSchema } from "./user";

const flag: boolean = boolean().parse(true);
const bad: string = boolean().parse(false);
const nested: (string | undefined)[] = nameSchema.optional().array().parse([]);

export { flag, bad, nested };
