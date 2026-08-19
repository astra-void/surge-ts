interface Task {
  name: string;
}
type TaskListener = (event: { task: Task }) => void;
interface EventsMap {
  cycle: TaskListener;
  error: TaskListener;
}
type Events = keyof EventsMap;

declare class Emitter {
  addEventListener(type: string, listener: (event: unknown) => void): void;
}

declare class Bench extends Emitter {
  addEventListener<K extends Events, T = EventsMap[K]>(type: K, listener: T): void;
}

export function listen(bench: Bench): void {
  bench.addEventListener("cycle", (e) => {
    void e.task.name;
  });
}
