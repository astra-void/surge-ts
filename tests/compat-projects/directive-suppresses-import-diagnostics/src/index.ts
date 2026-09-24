// @ts-ignore
import ignored from "not-installed";
// @ts-expect-error
import expected from "./missing";
import reported from "also-not-installed";
// @ts-ignore
import { absent } from "./present";
import { alsoAbsent } from "./present";

export const values = [ignored, expected, reported, absent, alsoAbsent];
