import * as MyLib from "mylib";

type ButtonPayload = MyLib.ElementProps<"button">;

const bad: ButtonPayload["onEvent"] = 123;

export const ok: ButtonPayload["onEvent"] = (value) => {
    void value;
};

export { bad };
