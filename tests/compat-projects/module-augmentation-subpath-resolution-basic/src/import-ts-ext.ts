import type { Tables } from 'pkg/types/tables.ts';
export const t: Tables = { base: "", extra: 1, more: 2 };
const bad: Tables = { base: 1, extra: 1, more: 2 };
export { bad };
