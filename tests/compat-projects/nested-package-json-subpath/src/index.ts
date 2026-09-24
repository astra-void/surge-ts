import { bar } from "foo/bar";
import { scoped } from "foo/@scoped";
import { inner } from "gated/inner";
import { sub } from "plain/sub";

export const values = [bar, scoped, inner, sub];
