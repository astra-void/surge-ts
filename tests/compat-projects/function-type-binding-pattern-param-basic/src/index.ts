import { Controller } from "formlib";

export const rendered = Controller({
  name: "email",
  render: ({ field }) => field.value,
});

const bad: number = Controller;

export { bad };
