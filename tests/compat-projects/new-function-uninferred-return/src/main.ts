new hoisted(1, "two");
new returnsNumber();
new annotatedVoid();

function hoisted(first: number, ...rest: string[]) {
}

function returnsNumber(): number {
    return 1;
}

function annotatedVoid(): void {
}
