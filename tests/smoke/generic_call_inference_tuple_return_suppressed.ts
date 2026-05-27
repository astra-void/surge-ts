function useState<T>(initial: T): [T, (next: T) => void] {
  return undefined as any;
}

const state = useState("x");

let first: number = state[0];
