export function handle(): string {
  try {
    throw new Error("bad");
  } catch (error) {
    console.error(error);
    return String(error);
  }
}

export function catchWithoutBinding(): string {
  try {
    return "ok";
  } catch {
    return "bad";
  }
}

export function noLeak(): void {
  try {
    throw new Error("bad");
  } catch (error) {
    console.error(error);
  }
  console.error(error);
}
