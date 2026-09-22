export {};

try {
} catch (error: string) {}
try {
} catch (error: unknown) {}
try {
} catch (error) {
  let error = 1;
}
try {
} catch (error) {
  var error = 2;
}

function notGenerator() {
  yield 1;
}
function* generator(x = yield 1): Generator<number, void, unknown> {}
async function asyncDefault(x = await 1) {}
class StaticBlock {
  static {
    await 1;
  }
}
function notAsync() {
  for await (const x of []) {
  }
}
async function isAsync() {
  for await (const x of []) {
  }
}
