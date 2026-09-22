type CallbackOrValue<T> = T | (() => T | Promise<T>);

export const resultOf = <T,>(value: T | (() => T)): T =>
  typeof value === 'function' ? (value as () => T)() : value;

export async function prepareUrl(options: { url: CallbackOrValue<string>; params: boolean }) {
  const url = await resultOf(options.url);
  if (!options.params) return url;
  return url + '?params=1';
}
