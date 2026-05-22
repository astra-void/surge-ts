import { verify } from "pkg";

export async function run() {
  const verification = await verify();
  return verification.verified;
}
