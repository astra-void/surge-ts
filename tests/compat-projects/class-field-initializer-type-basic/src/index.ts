class Counter {
  count = 0;
  label = "counter";
  enabled = false;
  offset = -1;
  title = `plain`;
  steps = [1, 2, 3];
  readonly kind = "counter";
  static instances = 0;
  static readonly version = 2;

  bump() {
    this.count = "one";
  }
}

const counter = new Counter();
const count: string = counter.count;
const label: number = counter.label;
const enabled: string = counter.enabled;
const offset: string = counter.offset;
const title: number = counter.title;
const steps: string[] = counter.steps;
const kind: "other" = counter.kind;
const instances: string = Counter.instances;
const version: 3 = Counter.version;

const widenedCount: number = counter.count;
const literalKind: "counter" = counter.kind;

class Store {
  state = { count: 0, label: "x", nested: { on: true } };
  readonly config = { mode: "a" };
  empty = {};

  reset() {
    this.state.count = "zero";
  }
}

const store = new Store();
const stateCount: string = store.state.count;
const nestedOn: number = store.state.nested.on;
const mode: "b" = store.config.mode;
store.empty.extra = 1;
const widenedMode: string = store.config.mode;
