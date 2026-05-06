import defaultValue, { GET, POST, type User } from "./source";
import type * as SourceTypes from "./source";

const user: User = { id: "1" };
const ok: boolean = defaultValue.ok;
const getResult: string = GET();
const postResult: string = POST();

void user;
void ok;
void getResult;
void postResult;
