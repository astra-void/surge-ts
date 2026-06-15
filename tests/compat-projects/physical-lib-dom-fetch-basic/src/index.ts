async function main() {
  const response = await fetch("/api");
  const ok: boolean = response.ok;
  const bad: number = response.ok;
}
