declare const iter: NS.Iter<string>;
declare const holder: Holder;
declare const checked: Checked;
declare const badlyChecked: Checked<number>;
export const a: number = iter.first;
export const b: string = holder.held;
export const c: number = checked.again;

interface Local<T> { local: T }
interface Local<T = boolean> { other: T }
declare const local: Local;
export const d: string = local.other;
