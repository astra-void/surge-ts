export interface StoreApi<T> {
  setState: {
    (partial: T | Partial<T>, replace?: false): void
    (state: T, replace: true): void
  }
}

type StoreDevtools<S> = S extends {
  setState: {
    (...args: infer Sa1): infer Sr1
    (...args: infer Sa2): infer Sr2
  }
}
  ? {
      setState(...args: [...args: Sa1, action?: string]): Sr1
      setState(...args: [...args: Sa2, action?: string]): Sr2
      devtools: { cleanup: () => void }
    }
  : never

type Impl = <T>(fn: () => T, api: StoreApi<T>) => void
export const install: Impl = (fn, api) => {
  type S = ReturnType<typeof fn> & { [store: string]: ReturnType<typeof fn> }
  ;(api as StoreApi<S> & StoreDevtools<S>).devtools = { cleanup: () => {} }
}
