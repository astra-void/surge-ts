import "configlib";

declare module "configlib" {
  interface Registry<T> {
    has(key: string): boolean;
  }
  const extras: { timeout: number };
}
