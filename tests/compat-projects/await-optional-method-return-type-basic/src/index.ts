interface Passkey {
  id: string;
}

interface Adapter {
  getPasskeyByRaw?: (raw: string) => Promise<Passkey | null>;
}

export async function run(
  adapter: Adapter,
  raw: string,
): Promise<{ passkey: Passkey; verification: unknown } | undefined> {
  const dbPasskey = await adapter.getPasskeyByRaw?.(raw);
  if (!dbPasskey) {
    return undefined;
  }

  const verification = {};
  return { verification, passkey: dbPasskey };
}

export async function badRun(
  adapter: Adapter,
  raw: string,
): Promise<{ passkey: Passkey; verification: unknown } | undefined> {
  const dbPasskey = await adapter.getPasskeyByRaw?.(raw);
  if (!dbPasskey) {
    return undefined;
  }

  return 123;
}
