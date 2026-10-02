// The declared name resolves inside a nested function body in its own
// initializer; a direct self-read stays a temporal-dead-zone error.
export function schedule() {
const timer = setInterval(() => { clearInterval(timer); }, 10);
return timer;
}
