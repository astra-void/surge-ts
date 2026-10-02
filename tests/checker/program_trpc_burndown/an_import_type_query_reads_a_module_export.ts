// @filename: node_modules/dep/index.d.ts
export declare const count: number;
// @filename: src/index.ts
declare const c: typeof import('dep')['count']; const s1: string = c; declare const q: typeof import('dep').count; const s2: string = q; const s3: string = null as unknown as typeof import('dep')['count'];
