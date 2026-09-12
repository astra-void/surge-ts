import type { RouterBase, RouterRecord } from './types';

type IntersectionError<K extends string> = `collision:${K}`;

export type ProtectedIntersection<TType, TWith> = keyof TType & keyof TWith extends never
  ? TType & TWith
  : IntersectionError<string & keyof TType & keyof TWith>;

export type CreateNext<TRouter extends RouterBase> = ProtectedIntersection<TRouter, RouterRecord>;

export declare function createNext<TRouter extends RouterBase>(): CreateNext<TRouter>;
