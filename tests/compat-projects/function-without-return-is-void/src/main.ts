function noReturn() {
  const local = 1;
}
function onlyThrows() {
  throw new Error("unreachable end");
}
function returnsInNested() {
  const inner = () => {
    return 1;
  };
}

export const value: number = noReturn();
export const thrown: number = onlyThrows();
export const nested: number = returnsInNested();
export const constructed = new noReturn();
