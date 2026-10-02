interface Reader<TValue> { read<TResult>(map: (value: TValue) => TResult): TResult; } declare const reader: Reader<string>; const width: number = reader.read((value: string) => value.length);
