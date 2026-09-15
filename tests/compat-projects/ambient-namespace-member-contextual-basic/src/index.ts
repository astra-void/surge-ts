export {}
globalThis.hostruntime = {
  streamifyResponse(handler) {
    return handler;
  },
  onError(handler) {
    handler(undefined);
  },
};
