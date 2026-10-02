// @filename: node_modules/dep/index.d.ts
interface Payload {
  kind: "dep";
  size: number;
}

export declare function getPayload(): Payload;
// @filename: src/index.ts
// The consumer's conflicting `Payload` must not leak into the dependency:
// `fromDep.size` is only a `number` if dep's own `Payload` was used.
import { getPayload } from "dep";
interface Payload { kind: "local"; size: string; }
const fromDep = getPayload();
const size: number = fromDep.size;
const local: Payload = { kind: "local", size: "here" };
export { size, local };
