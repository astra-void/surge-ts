export type Status = "pending" | "success" | "error";
declare const state: { status: Status };
declare const flag: boolean;
declare const rest: { parse(): string } | null;
declare const items: Array<{ parse(): string }>;
declare const index: number;

export function reassigned(): Status {
  let { status } = state;
  if (flag) {
    status = "success";
  }
  return status;
}

export function widenedLiteral(): string {
  const written = "a";
  let copied = written;
  copied = "z";
  return copied;
}

export function aliasGuard(): string {
  const schema = items[index] || rest;
  if (!schema) return "";
  return schema.parse();
}

export function mismatched(): number {
  let { status } = state;
  if (flag) {
    status = "success";
  }
  return status;
}
