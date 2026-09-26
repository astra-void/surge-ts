declare const resource: Disposable;
declare const asyncResource: AsyncDisposable;

export function patterns() {
    using {} = resource;
    for (using {} of [resource]) {
    }
}

export async function asyncPatterns() {
    await using {} = asyncResource;
    for (await using {} of [asyncResource]) {
    }
}

export function uninitialized() {
    using missing;
    for (using of;;) break;
}

export function clauses(kind: number) {
    switch (kind) {
        case 0:
            using inCase = resource;
            break;
        default: {
            using inBlock = resource;
        }
    }
}

export async function asyncClauses(kind: number) {
    switch (kind) {
        case 0:
            await using inCase = asyncResource;
            break;
    }
}

export function notAsync() {
    await using inPlainFunction = asyncResource;
    const arrow = () => {
        await using inArrow = asyncResource;
    };
    return arrow;
}

export class Holder {
    static {
        await using inStaticBlock = asyncResource;
    }
}

export async function allowed() {
    await using fine = asyncResource;
    using alsoFine = resource;
    const nested = async () => {
        await using nestedFine = asyncResource;
    };
    return nested;
}

export const constMissing: number;
