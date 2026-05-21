import { Adapter } from "./adapters";

export const adapter: Adapter = {
  getUser: async (id) => ({ id, role: "admin" }),
  getPasskeys: async (userId) => [{ id: "p1", userId, counter: 1 }],
};
