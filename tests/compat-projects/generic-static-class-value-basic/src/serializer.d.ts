type Transformer<I, O> = { name: string; serialize(value: I): O };

export default class Serializer {
  serialize(value: unknown): string;
  static serialize: (value: unknown) => string;
  static deserialize: <T = unknown>(payload: string) => T;
  static register: <I, O extends string>(transformer: Omit<Transformer<I, O>, 'name'>) => void;
}
