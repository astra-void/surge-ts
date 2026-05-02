let left: [string, number] = ["Ada", 36];
let right: [string, number[]] = ["Grace", [1, 2]];
let pair: [string, number] | [string, number[]] = true ? left : right;
