import { AdapterUser } from "./adapters";

export const missingId: AdapterUser = { role: "admin" };
export const wrongId: AdapterUser = { id: 123 };
