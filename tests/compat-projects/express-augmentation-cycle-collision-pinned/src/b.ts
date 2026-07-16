import { connect } from "storekit";

type Store = { kind: "ephemeral" };

const conn = connect();
const kind: "persistent" = conn.store.kind;
const bad: Store = conn.store;
void bad;

export { kind };
