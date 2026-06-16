export function useNode(): string | undefined {
  const encoded = Buffer.from("payload");
  const home = process.env.HOME;
  const dir = process.cwd();
  return home ?? dir ?? encoded;
}
