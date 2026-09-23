export {};

export function createClient(options = defaultOptions()) { return options.retries; }
function defaultOptions() { return { retries: 3 }; }

export function container() {
    function nested(x: typeof nested): typeof sibling { return sibling; }
    function sibling(x = nested) { return x; }
    return [nested, sibling];
}

export function wrongOptions(options = laterOptions()) { const s: string = options.retries; return s; }
function laterOptions() { return { retries: 3 }; }
