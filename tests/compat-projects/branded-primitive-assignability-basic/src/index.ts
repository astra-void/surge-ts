declare const marker: "lazy" & { __brand: "lazy" };
type UserId = string & { readonly __tag: unique symbol };
declare const userId: UserId;
declare const plainText: string;
declare const pair: { a: 1 } & { b: 2 };

const markerAsString: string = marker;
const markerAsLiteral: "lazy" = marker;
const userIdAsString: string = userId;

const markerAsNumber: number = marker;
const userIdAsNumber: number = userId;
const plainAsUserId: UserId = plainText;
const pairAsString: string = pair;
