interface Result {
  mean: number;
}
interface Task {
  result?: Result;
}

export function compare(a: Task, b: Task): number {
  if (!a.result || !b.result) throw new Error("no result");
  return a.result?.mean - b.result?.mean;
}

export function unguarded(a: Task): number {
  const mean: number = a.result?.mean;
  return mean;
}
