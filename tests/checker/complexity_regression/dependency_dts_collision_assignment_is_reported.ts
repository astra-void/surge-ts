// @filename: node_modules/dep/index.d.ts
interface Payload {
  kind: "dep";
  size: number;
}

export declare function getPayload(): Payload;
// @filename: src/index.ts
import { getPayload } from "dep";
interface Payload { kind: "local"; size: string; }
const collided: Payload = getPayload();
export { collided };
