import type { ParsedQuery } from 'mypkg';
import { parse } from 'mypkg';

declare const query: ParsedQuery;
export const value: number = query.id;
export const parsed = parse('a=1');
