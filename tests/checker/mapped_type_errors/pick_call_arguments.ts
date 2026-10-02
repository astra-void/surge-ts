// @strict: true

interface State { name: string; count?: number; }
declare const state: State;
declare function update<T, K extends keyof T>(object: T, patch: Pick<T, K>): void;
update(state, { name: "ok" });
update(state, { count: undefined });
update(state, {});
update(state, state);
update(state, { name: undefined });
update(state, { missing: true });
class Store<T> {
    constructor(readonly state: T) {}
    update<K extends keyof T>(patch: Pick<T, K>) {}
}
const store = new Store(state);
store.update({ name: "ok" });
store.update({ count: undefined });
store.update({});
store.update({ name: undefined });
store.update({ missing: true });
