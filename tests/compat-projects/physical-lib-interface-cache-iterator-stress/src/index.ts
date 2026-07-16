function drain<T>(iterator: IterableIterator<T>): T[] {
  const values: T[] = [];
  for (const value of iterator) values.push(value);
  return values;
}

const source = [1, 2, 3];
const results = Array.from({ length: 200 }, () => drain(source.values()));
const value: number | undefined = results[0]?.[0];
void value;
