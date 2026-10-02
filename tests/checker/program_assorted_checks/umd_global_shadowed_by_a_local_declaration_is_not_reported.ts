// @filename: legacy.d.ts
export declare function greet(name: string): string;
export as namespace Legacy;
// @filename: consumer.ts
const Legacy = { greet: (name: string) => name };
export const greeting = Legacy.greet("x");
