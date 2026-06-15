export {};

interface Api {
  get(id: string): string;
}

interface Api {
  set(id: string, value: string): void;
}

declare const api: Api;

const value = api.get("x");
const ok: string = value;
const bad: number = value;

api.set("x", "y");
api.set("x", 123);
