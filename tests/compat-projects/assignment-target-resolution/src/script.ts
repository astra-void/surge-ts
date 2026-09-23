// An assignment's target resolves exactly as a read of the name does: a `var`
// is written from inside its own initializer, and a function body writes a
// binding the file declares after it.
var scriptSelf: any = (scriptSelf = 2);
var scriptChain: any = (scriptChain = scriptChain = 2);
function writesLater() { scriptCounter = 1; }
var scriptCounter = 0;

var scriptTyped: string = (scriptTyped = 1, "");
