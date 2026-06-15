interface CookieJar {
  get(name: string): string | undefined;
}

declare class User {
  get id(): string;
  get cookies(): CookieJar;
}
