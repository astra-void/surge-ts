export namespace Api {
    export interface Request { url: string }
}

export namespace Api {
    export interface Request { method: string }
    declare const r: Request;
    r.url; r.method;
    r.body;
}
