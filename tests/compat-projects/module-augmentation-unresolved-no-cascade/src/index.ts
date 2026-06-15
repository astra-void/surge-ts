import type { Thing } from "missing-pkg";

const value: Thing = {
  id: "x",
};

const bad = value.missing;
