// The package's `types` entry is a global script whose whole body is an ambient
// `declare module`, so the ambient declaration supplies the exports.
import { uneval } from 'amb-entry';
// A `types` entry that *is* a module keeps precedence over a same-named ambient
// declaration inside it.
import { fromRealModule } from 'real-entry';

export const text: string = uneval(1);
export const count: number = fromRealModule(1);
