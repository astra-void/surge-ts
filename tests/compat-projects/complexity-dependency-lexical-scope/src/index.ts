import { getPayload } from "dep";

interface Payload {
  kind: "local";
  size: string;
}

const fromDep = getPayload();
const size: number = fromDep.size;

const local: Payload = { kind: "local", size: "here" };

const collided: Payload = getPayload();

export { size, local, collided };
