type Action =
    | { kind: "A"; payload: number | undefined }
    | { kind: "B"; payload: string | undefined };

export function guarded({ kind, payload }: Action) {
    if (payload) {
        if (kind === "A") {
            payload.toFixed();
        }
        if (kind === "B") {
            payload.toUpperCase();
            payload.toExponential();
        }
    }
}

export function unguarded({ kind, payload }: Action) {
    if (kind === "A") {
        payload.toFixed();
    }
}

type Result = [error: null, data: string[]] | [error: Error, data: undefined];

export function readResult(result: Result) {
    const [error, data] = result;
    if (error === null) {
        data.length;
    } else {
        error.message;
        data.length;
    }
}
