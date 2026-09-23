type Args = ["text", string] | ["count", number];
declare function listen(handler: (...args: Args) => void): void;

listen((kind, value) => {
    const name: "text" | "count" = kind;
    if (kind === "text") {
        value.toUpperCase();
    } else {
        value.toFixed();
    }
});

listen((kind, value) => {
    value.toFixed();
});

type Optional = ["some", number] | ["none"];
declare function maybe(handler: (...args: Optional) => void): void;

maybe((kind, value?) => {
    if (kind === "some") {
        value.toFixed();
    } else {
        const nothing: undefined = value;
    }
});

const reassigned: (...args: [1, "a"] | [2, "b"]) => void = (tag, letter) => {
    if (Math.random()) {
        tag = 1;
    }
    if (tag === 1) {
        const exact: "a" = letter;
    }
};
