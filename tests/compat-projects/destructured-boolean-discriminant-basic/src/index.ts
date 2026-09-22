type Result<T> = { done: false; value: T } | { done: true; value?: undefined };
declare function next(): Result<string>;
declare function nextAsync(): Promise<Result<string>>;

export function falsyBranch() {
    const { done, value } = next();
    if (!done) {
        const s: string = value;
        return s;
    }
    return "";
}

export function earlyExit() {
    const { done, value } = next();
    if (done) {
        return "";
    }
    const s: string = value;
    return s;
}

export function truthyBranch() {
    const { done, value } = next();
    if (done) {
        const s: string = value;
        return s;
    }
    return "";
}

export async function inLoop() {
    let buffer = "";
    while (true) {
        const { done, value } = await nextAsync();
        if (done) break;
        buffer += value;
    }
    return buffer;
}
