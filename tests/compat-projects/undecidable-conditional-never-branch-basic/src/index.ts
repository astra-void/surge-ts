type DropLast<T extends ReadonlyArray<unknown>> = T extends readonly [...infer R, unknown]
  ? readonly [...R]
  : never;

type TuplePrefixes<T extends ReadonlyArray<unknown>> = T extends readonly []
  ? readonly []
  : TuplePrefixes<DropLast<T>> | T;

type K = readonly ['key', { a: number; b: string }];

export const prefix: TuplePrefixes<K> = ['key'] as const;
export const empty: TuplePrefixes<K> = [] as const;
export const whole: TuplePrefixes<K> = ['key', { a: 1, b: 'b' }] as const;
