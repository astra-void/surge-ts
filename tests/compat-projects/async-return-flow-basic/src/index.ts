export async function directReturn(): Promise<string> {
  return "ok";
}

export async function awaitedReturn(): Promise<string> {
  const value = await Promise.resolve("ok" as string);
  return value;
}

export async function branchReturn(flag: boolean): Promise<string | null> {
  if (flag) return "yes";
  return null;
}

export async function tryCatchReturn(flag: boolean): Promise<string | null> {
  try {
    if (flag) return "yes";
    return null;
  } catch {
    return null;
  }
}
