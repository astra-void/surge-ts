type JSONSchema = { [k: string]: unknown; $defs?: Record<string, JSONSchema> };

declare const external: { defs: Record<string, JSONSchema> } | undefined;

function size(defs: Record<string, JSONSchema>): number {
  return Object.keys(defs).length;
}

export function collectDefs(): number {
  const defs: JSONSchema["$defs"] = external?.defs ?? {};
  return size(defs);
}

export function collectValues(values: unknown[] | undefined, fallback: unknown[]): number {
  const all: unknown[] | undefined = values ?? fallback;
  return all.length;
}

export function collectUnnarrowed(defs: Record<string, JSONSchema> | undefined): number {
  return size(defs);
}
