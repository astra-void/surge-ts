// @filename: globals.d.ts
interface Response { json(): Promise<any>; }
// @filename: node_modules/node-fetch/@types/index.d.ts
export type HeadersInit = Record<string, string>;
export type BodyInit = string;
export interface RequestInit { body?: BodyInit; headers?: HeadersInit; method?: string; }
export type RequestInfo = string | Request;
declare class BodyMixin {
constructor(body?: BodyInit, options?: { size?: number });
readonly body: NodeJS.ReadableStream | null;
json(): Promise<unknown>;
}
export class Request extends BodyMixin {}
export class Response extends BodyMixin {}
export default function fetch(url: URL | RequestInfo, init?: RequestInit): Promise<Response>;
// @filename: src/index.ts
import fetch from 'node-fetch';
fetch('https://mds3.fido.tools/getEndpoints', {
method: 'POST',
body: JSON.stringify({ endpoint: 'https://example.com' }),
headers: { 'Content-Type': 'application/json' },
})
.then((resp) => resp.json())
.then((json) => { const mdsServers: string[] = json.result; });
// @filename: node_modules/node-fetch/package.json
{ "name": "node-fetch", "types": "@types/index.d.ts" }
