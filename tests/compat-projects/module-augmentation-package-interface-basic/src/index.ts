import type { Client } from "pkg";

const ok: Client = {
  id: "c1",
  token: "t",
};

const missing: Client = {
  id: "c1",
};

const bad: Client = {
  id: "c1",
  token: 123,
};
