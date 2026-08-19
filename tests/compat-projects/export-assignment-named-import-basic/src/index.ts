import { stat } from 'classmod';
import { Bar, helper } from 'nsmod';
import { stat as aliasStat } from 'aliasmod';
import type { Opts } from 'classmod';
import type { Opts as AliasOpts } from 'aliasmod';
import type { Baz } from 'nsmod';

export const values = [stat, Bar, helper, aliasStat];
export const opts: Opts = { a: 1 };
export const aliasOpts: AliasOpts = { a: 2 };
export const baz: Baz = { z: 3 };
