declare const flag: boolean;

export function bareReturn(): number {
    if (flag) return;
}

export function onlyThrow(): number {
    if (flag) throw new Error();
}

export function valueReturn(): number {
    if (flag) return 1;
}
