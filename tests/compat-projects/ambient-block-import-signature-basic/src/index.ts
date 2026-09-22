import { access, stat } from "m:fsp";

access("a");
access(1);
export const size: Promise<string> = stat("a").then((s) => s.size);
