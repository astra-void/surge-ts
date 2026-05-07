export async function requestJson(
  input: string,
  method: "GET" | "POST" = "GET",
  body?: object
): Promise<object | null> {
  return body ?? null;
}

export function optionalAnnotated(
  req?: { cookies: { get(name: string): string | undefined } },
  status?: number,
  body?: object
): object {
  return { req, status, body };
}

export function optionalDefaulted(
  status: number = 500,
  message?: string
): string {
  return `${status}:${message ?? ""}`;
}

export function shouldStillReport(value) {
  return value;
}
