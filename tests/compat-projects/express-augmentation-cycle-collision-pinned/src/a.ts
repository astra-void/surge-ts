import { connect } from "storekit";

interface Store {
  kind: "memory";
  flush(): void;
}

const conn = connect();
const kind: "persistent" = conn.store.kind;
const reentered: "persistent" = conn.store.connection().store.kind;
const local: Store = { kind: "memory", flush() {} };
const bad: "memory" = conn.store.kind;
void bad;

export { kind, reentered, local };
