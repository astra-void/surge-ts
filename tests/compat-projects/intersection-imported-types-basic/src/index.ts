import type { User, Named } from "./types";

type Profile = User & Named;

const ok: Profile = { id: "u1", name: "Ada" };
const bad: Profile = { id: "u1" };
