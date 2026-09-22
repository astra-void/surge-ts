export {};

declare const flag: boolean;
while (flag) const inLoop = 1;
if (flag) const inThen = 2;
else const inElse = 3;

let initialized!: number = 1;
let untyped!;
declare let ambient!: number;
class Fields {
  initialized!: number = 1;
  asserted!: number;
}

const getters = {
  get x() {
    return 1;
  },
  get x() {
    return 2;
  },
  a: 1,
  a: 2,
};
const setters = {
  set x(value: number) {},
  set x(value: number) {},
};
const pair = {
  get x() {
    return 1;
  },
  set x(value: number) {},
};
