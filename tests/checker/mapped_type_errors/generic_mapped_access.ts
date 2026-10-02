// @strict: true

function pick<T, K extends keyof T>(object: Pick<T, K>) { object.missing; }
function record<K extends string>(object: Record<K, number>) { object.missing; }
type Invalid<T> = { [P in T]: T[P] };
type Valid<T, K extends keyof T> = { [P in K]: T[P] };
function homomorphic<T extends { name: string }>(object: { [P in keyof T]: T[P] }) {
    object.name;
    object.missing;
}
function fixed<K extends "name">(object: Record<K, number>) { object.name; }
function optional<T extends { name: string }>(object: Partial<T>) {
    const name: string | undefined = object.name;
}
function required<T extends { name?: string }>(object: Required<T>) {
    const name: string = object.name;
}
