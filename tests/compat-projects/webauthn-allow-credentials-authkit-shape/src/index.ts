interface StoredPasskey {
  id: string;
  transports: string;
}

interface AllowCredential {
  id: string;
  transports?: AuthenticatorTransport[];
}

declare function generateAuthenticationOptions(options: {
  allowCredentials?: AllowCredential[];
}): void;

export function run(passkeys: StoredPasskey[]) {
  const allowCredentials = passkeys.map((passkey) => ({
    id: passkey.id,
    transports: passkey.transports.split(",") as AuthenticatorTransport[],
    type: "public-key",
  }));

  generateAuthenticationOptions({
    allowCredentials,
  });
}

export function wrong(passkeys: StoredPasskey[]) {
  const allowCredentials = passkeys.map((passkey) => ({
    id: passkey.id,
    transports: passkey.transports.split(","),
  }));

  generateAuthenticationOptions({
    allowCredentials,
  });
}
