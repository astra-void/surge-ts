export function sequentialLocals(input: string): string {
  const cleaned = input.replace(/=+$/, "").toUpperCase();
  const parts = cleaned.split(":");
  const timestampStr = parts[0];
  const timestamp = Number(timestampStr);
  if (isNaN(timestamp)) return "";
  const result = timestamp.toString().padStart(6, "0");
  return result;
}

export function blockLocals(flag: boolean): string {
  if (flag) {
    const value = "a";
    return value.replace("a", "b");
  }

  const other = "c";
  return other.toUpperCase();
}

export function branchThenLater(flag: boolean): string {
  let value = "";
  if (flag) {
    value = "x";
  }
  return value;
}
