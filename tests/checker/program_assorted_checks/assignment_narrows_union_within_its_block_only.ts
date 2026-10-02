// Assigning to a union-declared variable narrows it (the lazy-singleton idiom),
// but only within the block that assigned — a branch assignment must not leak.
//
// The `pick` half also pins literal-equality narrowing: `target` is
// `"draft-07"` on both edges into the second test, so tsc 7.0.2 reports
// `TS2367` there (verified against the oracle at the same file/code/line/column
// and message text). This assertion held `[]` while surge had no
// literal-equality narrowing at all.
interface Client { id: number }
declare function createClient(): Client;
let client: Client | null = null;
function get(): Client {
if (client) return client;
client = createClient();
return client;
}
type Target = "draft-04" | "draft-07";
function pick(input: Target): Target {
let target: Target = input;
if (target === "draft-04") target = "draft-07";
if (target === "draft-04") return target;
return target;
}
export const use = [get, pick];
