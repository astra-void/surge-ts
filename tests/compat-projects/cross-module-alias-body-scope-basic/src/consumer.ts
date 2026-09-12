import type { Factory } from './lib';

export type Record1<A, B> = { a: A; b: B };

declare const factory: Factory;

export const caller = factory.create();
export const result: { id: number } = caller(1);
export const named = factory.createFor({ name: 'x' })(2).name;

export const local: Record1<string, number> = { a: '', b: 1 };

export const wrongArity: Record1<string> = { a: '', b: 1 };
