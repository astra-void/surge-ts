import { greet } from "./legacy";

const Legacy = { greet };

export const shadowed = Legacy.greet("shadowed");

export const planner = {
  render: (path: string) => {
    // A function-local binding shadows the UMD global just as a module-level
    // one does, so neither read below is a TS2686.
    const Legacy = path.toString();
    return Legacy && `?${Legacy}`;
  },
};
