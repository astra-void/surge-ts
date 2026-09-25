export type Place = "sky" | "roof" | "garage";

type Before = {
  model: "hour" | "day";
  [place in Place]: void;
};

type After = {
  [place in Place]: void;
  model: "hour" | "day";
};

type AfterMethod = {
  [place in Place]?: void;
  model(duration: number): "hour" | "day";
};

interface Declared {
  [P in Place]: unknown;
}

class Holder {
  [P in Place]: unknown;
}

export type { Before, After, AfterMethod, Declared };
export { Holder };
