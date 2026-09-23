declare const text: string;
declare const nothing: undefined;

export const count: number = text && 1;
export const none: undefined = nothing && 1;
export const mixed: number = text && "";
