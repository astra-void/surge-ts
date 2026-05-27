function useState<T>(initial: T): [T, (next: T) => void] {
  return undefined as any;
}

const state = useState<string>("x");

let first: string = state[0];
