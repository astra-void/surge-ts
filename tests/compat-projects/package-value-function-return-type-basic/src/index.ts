import { verify } from "pkg";
import type { Verification } from "pkg";

interface Passkey {
  id: string;
}

export async function run(): Promise<{ verification: Verification; passkey: Passkey } | undefined> {
  const verification = await verify();
  const passkey: Passkey = { id: "p1" };

  return { verification, passkey };
}
