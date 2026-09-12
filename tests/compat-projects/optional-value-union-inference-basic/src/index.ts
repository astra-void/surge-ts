interface Query {
  id: number
}

declare function resolveValue<TValue>(
  value: undefined | TValue | ((query: Query) => TValue),
  query: Query,
): TValue | undefined

type BooleanOption = boolean | ((query: Query) => boolean)

interface Options {
  enabled?: BooleanOption
  retryOnMount?: BooleanOption
}

export function shouldLoad(query: Query, options: Options): boolean {
  return (
    resolveValue(options.enabled, query) !== false &&
    resolveValue(options.retryOnMount, query) !== false
  )
}
