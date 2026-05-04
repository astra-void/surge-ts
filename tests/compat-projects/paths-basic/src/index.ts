import { User } from "@models";
import { makeUser } from "@app/makeUser";
import type { User as LibUser } from "@lib/user";
import { missing } from "@lib/user"; // TS2305
import { unresolvable } from "@unresolvable/path"; // TS2307
import "@app/sideEffect";

export const user: User = makeUser();
export const num: number = user; // TS2322
