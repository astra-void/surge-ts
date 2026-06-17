// Generated from the local TypeScript lib sources. Do not edit by hand.

interface TextEncoder {
  encode(input?: string): Uint8Array;
}

declare function TextEncoder(): TextEncoder;

type AuthenticatorTransport =
  | "ble"
  | "cable"
  | "hybrid"
  | "internal"
  | "nfc"
  | "smart-card"
  | "usb"
;

interface Crypto {
  getRandomValues(array: Uint8Array): Uint8Array;
}

interface Headers {}

interface Request {}

interface Response {
  ok: boolean;
  status: number;
  json(): unknown;
}

interface URL {}

interface Console {
  log: any;
  warn: any;
  error: any;
}

declare function fetch(input: unknown, init?: unknown): Promise<Response>;

declare function Headers(init?: unknown): Headers;
declare function Request(input?: unknown, init?: unknown): Request;
declare function Response(body?: unknown, init?: unknown): Response;
declare function URL(url: string): URL;
declare const crypto: Crypto;
declare const console: Console;

declare const globalThis: {
  crypto: Crypto;
};
