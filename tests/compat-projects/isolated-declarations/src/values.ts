declare function random(): number;
export const literal = 1;
export const computed = 1 + 1;
export const called = random();
export let widened = 1;
export let list = [1, 2, 3];
export const tuple = [1, 2, 3] as const;
export const spread = [1, ...tuple] as const;
const part = { z: 1 };
export const object = {
    a: 1,
    b: random(),
    ...part,
    part,
    [random()]: 1,
    method() {},
    ok(): void {},
};
export default random();
const hidden = random();
export const destructured = { x: 1 };
export const { x } = destructured;
