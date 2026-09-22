import Serializer from './serializer';

export const assigned: number = Serializer;

declare function takesNumber(value: number): void;
takesNumber(Serializer);

export function returned(): number {
  return Serializer;
}

class Local {
  static parse<T>(text: string): T {
    return JSON.parse(text) as T;
  }
}
export const local: string = Local;
