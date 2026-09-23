import { decorate } from './lib';
import { derived } from './derived';
export const client = decorate<typeof derived>();
export const annotated: typeof derived = derived;
