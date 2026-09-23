var count = 10;
switch (count) {
    case 5:
        count++;
        break;
    default:
        count = count * 10;
}

function f(): number {
    var total: number = 1;
    switch (total) {
        case 2:
            break;
        default:
            total = 3;
    }
    {
        var declaredInBlock = "visible after the block";
    }
    return declaredInBlock.length + total;
}

var label = 1;
switch (label) {
    case 1:
        break;
    default:
        label = "not a number";
}
