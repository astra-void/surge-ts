export const shared = 1;
export default shared + 1;

export async function load() {
  return import("./dep", { with: { type: "json" } });
}
