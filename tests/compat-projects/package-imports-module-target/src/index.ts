import { fromSelf } from "#self";
import { fromDep } from "#dep";
import { fromDepSub } from "#dep/sub";
import { missing } from "#missing";
import { parent } from "#parent";

export const values = [fromSelf, fromDep, fromDepSub, missing, parent];
