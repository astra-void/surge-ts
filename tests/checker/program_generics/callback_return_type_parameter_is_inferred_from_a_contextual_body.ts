// @surge-compare: messages
interface Reader<TValue> { read<TResult>(map: (value: TValue) => TResult): TResult; } declare const reader: Reader<string>; const width: number = reader.read((value) => value.length); const rejected: string = reader.read((value) => value.length);
