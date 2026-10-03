type X1<T> =
    T extends { a: infer U extends string, b: infer U extends number } ? U :
    never;
type X2<T> =
    T extends { a: infer U extends string, b: infer U extends string } ? U :
    never;
