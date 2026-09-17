export {}
interface Animal { legs: number }
interface Dog extends Animal { bark(): void }
declare const s: string;
declare const n: number;
declare const u: unknown;
declare const a: any;
declare const obj: { x: number };
declare const wider: { x: number; y: string };
declare const lit: "a" | "b";
declare const animal: Animal;
declare const dog: Dog;

const ok1 = u as string;
const ok2 = a as string;
const ok3 = s as any;
const ok4 = s as unknown;
const ok5 = animal as Dog;
const ok6 = dog as Animal;
const ok7 = wider as { x: number };
const ok8 = lit as "a";
const ok9 = lit as "z";
const ok10 = s as string;
const ok11 = obj as unknown as string;

const bad1 = s as number;
const bad2 = n as string;
const bad3 = obj as { x: string };
const bad4 = s as { x: number };
const bad5 = obj as number;
