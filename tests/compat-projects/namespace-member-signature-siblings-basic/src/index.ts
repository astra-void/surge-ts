import { useBox } from "../vendor/hooks";

declare const initial: number;

const setBox = useBox(initial);

setBox((previous) => previous + 1);

export const wrong: number = setBox;
