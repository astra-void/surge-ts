let constString = "a" as const;
constString = "b";

let constNumber = 1 as const;
constNumber = 2;

let constObject = { k: "a" } as const;
constObject = { k: "b" };

let assertedLiteral = "q" as "q";
assertedLiteral = "r";

let assertedObject = { k: "a" } as { k: "a" };
assertedObject = { k: "b" };

function parameterDefault(x = "a" as const) {
  x = "b";
}

function localLet() {
  let local = true as const;
  local = false;
}

let fresh = "a";
fresh = "b";

const declared = "a";
let copied = declared;
copied = "z";
