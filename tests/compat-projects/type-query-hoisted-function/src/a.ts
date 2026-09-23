type Later = ReturnType<typeof later>;
function early(): Later {
    return 1;
}
function later(): number {
    return 1;
}
interface Box {
    value: ReturnType<typeof make>;
}
function takesBox(box: Box): number {
    return box.value;
}
function make(): number {
    return 1;
}
const wrongLater: Later = "x";
const wrongBox: Box = { value: "s" };
