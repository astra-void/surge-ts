export function read<T extends object, K extends keyof T>(o: T, k: K): T[K] {
  return o[k];
}
