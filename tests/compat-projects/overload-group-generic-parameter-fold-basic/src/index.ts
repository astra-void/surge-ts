type Defined<TData, TKey> = { key: TKey; fn: () => TData; initialData: TData };
type Undefined<TData, TKey> = { key: TKey; fn: () => TData };

declare function query<TData = unknown, TKey extends string = string>(
  options: Defined<TData, TKey>,
): { data: TData };
declare function query<TData = unknown, TKey extends string = string>(
  options: Undefined<TData, TKey>,
): { data: TData | undefined };

export const fromTheFirstOverload = query({
  key: 'k',
  fn: () => 'v',
  initialData: 'v',
});

export const fromTheSecondOverload = query({
  key: 'k',
  fn: () => 'v',
});

export const withExplicitTypeArguments = query<string>({
  key: 'k',
  fn: () => 'v',
});
