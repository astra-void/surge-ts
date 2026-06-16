import { generateAuthenticationOptions } from "./lib/server";

type AuthenticatorTransport = "ble" | "hybrid" | "internal" | "nfc" | "usb";

interface StoredPasskey {
  webAuthnId: string;
  transports: string;
}

export async function loginOptions(
  passkeys: StoredPasskey[],
  mode: "email" | "credential",
): Promise<void> {
  let allowCredentials: Array<{
    id: string;
    type?: string;
    transports: AuthenticatorTransport[];
  }> = [];

  if (mode === "email") {
    allowCredentials = passkeys.map((p) => ({
      id: p.webAuthnId,
      transports: p.transports.split(",") as AuthenticatorTransport[],
    }));
  }

  await generateAuthenticationOptions({
    rpID: "example.com",
    allowCredentials,
  });
}
