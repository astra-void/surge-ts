declare const known: number | undefined;
function withDefault(value = known) {
    return value;
}
function withPattern([first = 0] = [known]) {
    return first;
}
function withMissing(value = unknownName) {
    return value;
}
withDefault();
withPattern();
withMissing();
