declare const ambientValue: number = 1;

declare namespace Ambient {
  const inside: number = 2;
}

export declare class AmbientClass {
  label = 'x';
}

export class Plain {
  abstract render(): void;
  abstract label: string;

  set size(next: number): void {
    void next;
  }

  set pair(first: number, second: number) {
    void first;
    void second;
  }
}

export abstract class Proper {
  abstract render(): void;
  abstract label: string;

  set size(next: number) {
    void next;
  }
}

export const accessorThenProperty = {
  get value() {
    return 1;
  },
  value: 2,
};

export const propertyThenAccessor = {
  value: 1,
  get value() {
    return 2;
  },
};

export { ambientValue, Ambient };
