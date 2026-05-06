export default { ok: true };
export type User = { id: string };
export interface Profile {
  name: string;
}
export const handler = () => "ok";
export { handler as GET, handler as POST };
