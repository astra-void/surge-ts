export class Factory {
  x = 1;
  static create = (seed?: number): Factory => new Factory();
  static withDefault = (count = 3): number => count;
  static generic = <T>(value: T): T[] => [value];
  static plain = function (label: string): string {
    return label;
  };
  build = (label: string): string => label;
}

export const createFactory = Factory.create;
