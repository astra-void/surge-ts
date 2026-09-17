declare const isReady: () => boolean;
declare const maybeCallback: (() => void) | undefined;
declare const service: {
  start(): void;
  stop?: () => void;
  nested: { run(): void };
};
function declared() {}

if (isReady) {
}
if (declared) {
}
if (service.start) {
}
if (service.nested.run) {
}
if (service && service.start) {
}
if (isReady || declared) {
}

if (isReady) {
  isReady();
}
if (isReady && isReady()) {
}
if (service.nested.run) {
  service.nested.run();
}
if (maybeCallback) {
}
if (service.stop) {
}
if (!isReady) {
}

function local(callback: () => void) {
  if (callback) {
    return;
  }
}

class Widget {
  render() {}
  update() {
    if (this.render) {
    }
  }
}
