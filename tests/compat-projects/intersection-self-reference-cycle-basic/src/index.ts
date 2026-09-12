type Procedure = (...args: any[]) => any
type Normalized<T extends Procedure> = T extends Procedure
  ? (...args: Parameters<T>) => ReturnType<T>
  : never

interface Spy<T extends Procedure> {
  mockImplementation(fn: Normalized<T>): this
}

declare function spyOn<T extends object, K extends keyof T>(
  object: T,
  key: K,
  accessor: 'get',
): Spy<() => T[K]>

const windowSpy = spyOn(globalThis, 'window', 'get')
windowSpy.mockImplementation(() => undefined as unknown as Window & typeof globalThis)

declare function normalized<T extends Procedure>(
  fn: (...args: Parameters<T>) => ReturnType<T>,
): void
normalized<() => Window & typeof globalThis>(
  () => undefined as unknown as Window & typeof globalThis,
)

const w = undefined as unknown as Window & typeof globalThis
export const inner: Window = w.window.self.window
export const doc: Document = w.self.document
