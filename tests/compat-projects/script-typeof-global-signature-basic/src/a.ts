function takesLater(x: typeof later): void;
function takesLater(x: any) { }
takesLater({ foo: "" });
takesLater({ foo: 1 });

var declaredHere: { bar: number };
function takesDeclared(x: typeof declaredHere) { }
takesDeclared({ bar: "" });

function takesNothing(x: typeof nowhere) { }
