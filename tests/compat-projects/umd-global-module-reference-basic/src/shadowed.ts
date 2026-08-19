import { greet } from "./legacy";

const Legacy = { greet };

export const shadowed = Legacy.greet("shadowed");
