import * as values from "./values";

values.fixed = 1;
values.counter = 1;
values.run = () => {};
values.missing = 1;
values["counter"] = 2;

export function inBody() {
    values.counter = 3;
    values.absent = 3;
}

export function shadowed(values: { counter: number }) {
    values.counter = 4;
}

values.counter++;
values["counter"]--;
for (values.counter of [1, 2]) {}

export function updates() {
    values.counter++;
    for (values.counter of [3]) {}
}
