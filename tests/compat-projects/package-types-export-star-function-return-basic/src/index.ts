import { run, Result } from "pkg";

export async function ok(): Promise<Result> {
  const value = await run();
  return value;
}

export async function bad(): Promise<Result> {
  return 123;
}
