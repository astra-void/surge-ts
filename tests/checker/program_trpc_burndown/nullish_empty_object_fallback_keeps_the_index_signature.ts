declare const rec: Record<string, { x: number }> | undefined; const props = rec ?? {}; const v = props['data']; const s: string = v;
