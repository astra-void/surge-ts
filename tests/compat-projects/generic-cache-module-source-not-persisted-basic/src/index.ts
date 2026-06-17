import type { Box as ABox } from "./a";
import type { Box as BBox } from "./b";

const a: ABox<string> = { item: "wrong" };
const b: BBox<string> = { value: "wrong" };

export { a, b };
