// @filename: types.ts
export namespace original { export interface Item { value: string; } }
// @filename: barrel.ts
import { original as local } from './types'; export { local as renamed };
// @filename: consumer.ts
import { renamed as ns } from './barrel'; const valid: ns.Item = { value: 'ok' }; const invalid: ns.Item = { value: 1 };
