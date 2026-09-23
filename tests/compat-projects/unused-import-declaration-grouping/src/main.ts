import all, { first as one, second as two } from "./a";
import some, { first, second } from "./a";
import single from "./a";
import { _ignored, second as used } from "./a";
import * as namespace from "./a";
export const read = [some, used];
