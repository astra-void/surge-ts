declare function isObject(v: unknown): v is Record<string, unknown>;

function message(err: unknown, fallback: string): string {
    if (isObject(err) && typeof err["message"] === "string") {
        return err["message"];
    }
    return fallback;
}

function slots(c: { [key: string]: string }) {
    c.a.length;
    if (c.a) {
        c.a.length;
    }
    c.b = "z";
    c.b.length;
    c.q.length;
    if (c["r"] !== undefined) {
        const s: string = c["r"];
        return s;
    }
    const t: string = c["r"];
    return t;
}
