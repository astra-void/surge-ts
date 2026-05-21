interface Provider {
  name: string;
  type: "credentials" | "passkey";
}

export function select(providers: Provider[], provider: string) {
  const selected = providers.find((p) => p.name === provider);
  const passkey = providers.find((p) => p.type === "passkey");
  return selected ?? passkey ?? null;
}

export function wrong(providers: Provider[]) {
  return providers.find((p) => p.missing === "x");
}

export function unknownFind(values: unknown[]) {
  const selected = values.find((value) => value !== null);
  return selected;
}
