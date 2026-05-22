import { verify } from "pkg/server";

export async function run() {
  const verification = await verify();
  return verification.verified;
}
