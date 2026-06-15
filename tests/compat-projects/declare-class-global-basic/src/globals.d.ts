declare class FetchHeaders {
  constructor(init?: Record<string, string>);
  get(name: string): string | null;
  set(name: string, value: string): void;
  static from(input: string): FetchHeaders;
}
