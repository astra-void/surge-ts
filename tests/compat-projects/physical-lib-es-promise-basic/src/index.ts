async function load(): Promise<string> {
  return "ok";
}

async function main() {
  const value = await Promise.resolve(1);
  const ok: number = value;
  const bad: string = value;
}
