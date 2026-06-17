import type { Box } from "pkg";

const ok: Box<number> = { value: 123 };
const b: Box<number> = { value: "wrong" };

export { ok, b };
