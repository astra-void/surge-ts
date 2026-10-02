// @filename: model.ts
export declare class Client<T = string, U = T> { value: U; }
// @filename: producer.ts
import { Client } from './model'; export const client = new Client();
// @filename: consumer.ts
import { client } from './producer'; const valid: string = client.value; const invalid: number = client.value; client.missing;
