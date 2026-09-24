export {};

let dup = 1;
var dup = 2;

interface Shape { size: number }
type Shape = { size: number };

enum Mode { A }
function Mode() {}

export default dup;
export default Mode;

var await = 1;

function strict(eval: number) { return eval; }

class Holder {
    method(arguments: number) { return arguments; }
    #constructor = 1;
}

let implements = 1;

label: function labeled() {}

const merged = { value() {}, get value() { return 1; } };
