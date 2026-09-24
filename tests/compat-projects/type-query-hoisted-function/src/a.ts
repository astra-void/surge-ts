type Later = ReturnType<typeof later>;
function early(): Later {
    return 1;
}
function wrong(): Later {
    return "x";
}
function later(): number {
    return 1;
}
interface Box {
    value: ReturnType<typeof make>;
}
function takesBox(box: Box): string {
    return box.value;
}
function make(): number {
    return 1;
}
const wrongLater: Later = "x";
const wrongBox: Box = { value: "s" };
function scoped() {
    type Inner = ReturnType<typeof innerLater>;
    function innerWrong(): Inner {
        return "x";
    }
    function innerLater(): number {
        return 1;
    }
    return innerWrong;
}
