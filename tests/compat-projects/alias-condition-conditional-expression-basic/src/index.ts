interface Response {
  id: number;
}

export function initResponse(opts: {
  json: Response | Response[] | null;
  meta?: (o: { data: Response[] }) => void;
}): Response[] {
  const { json, meta } = opts;
  const eager = !json;
  const data = eager ? [] : Array.isArray(json) ? json : [json];
  meta?.({ data });
  return data;
}

export function pickLabel(value: string | number | undefined): string {
  const isText = typeof value === 'string';
  return isText ? value.toUpperCase() : 'n/a';
}

export function bothOperands(a: string | null, b: number | undefined): number {
  const ready = a !== null && b !== undefined;
  return ready ? a.length + b : 0;
}

export function shadowRestores(value: string | null): number {
  const present = value !== null;
  {
    const present = value === null;
    if (present) {
      return 0;
    }
  }
  return present ? value.length : -1;
}

export function stillReports(value: string | undefined): number {
  const absent = value === undefined;
  return absent ? value.length : 0;
}
