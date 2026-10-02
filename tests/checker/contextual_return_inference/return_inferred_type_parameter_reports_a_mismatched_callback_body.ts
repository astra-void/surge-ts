// @noImplicitAny: true
interface Trait { tag: string }
interface Ctor<T extends Trait> { create(): T }
interface MyTrait extends Trait { alpha(n: number): void }
declare function make<T extends Trait>(name: string, init: (instance: T) => void): Ctor<T>;
const ctor: Ctor<MyTrait> = make("x", (instance) => {
const wrong: number = instance;
void wrong;
});
void ctor;
