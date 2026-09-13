declare const key: string;

export const literal = { a: 1, a: 2, a: 3 };

export const quoted = { b: 1, 'b': 2, ['b']: 3 };

const shorthandValue = 1;
export const shorthand = { shorthandValue, shorthandValue };

export const computed = { [key]: 1, [key]: 2 };

export const accessors = {
  get value() {
    return 1;
  },
  set value(next: number) {
    void next;
  },
};

export const distinct = { a: 1, b: 2 };
