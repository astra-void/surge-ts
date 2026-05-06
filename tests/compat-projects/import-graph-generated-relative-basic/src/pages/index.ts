import type { User } from "./types.gen";
import { auth } from "../core/auth.gen";
import { missing } from "./missing.gen";

export const currentUser: User = { name: "Ada" };
export const currentAuth = auth;
export const missingValue = missing;
