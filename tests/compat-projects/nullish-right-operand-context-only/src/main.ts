declare const maybe: { x: 1 } | undefined;
export const merged = maybe ?? { x: 5 };
declare const format: ((n: number) => string) | undefined;
export const formatter = format ?? ((n) => n.toFixed());
export const annotated: { x: 1 } = maybe ?? { x: 5 };
declare const nullable: { x: 1 } | null;
export const either = nullable || { x: 5 };
