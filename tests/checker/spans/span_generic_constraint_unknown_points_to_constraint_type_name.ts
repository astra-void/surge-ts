// @surge-compare: spans
type Box<T extends Missing> = { value: T }; let box: Box<string> = { value: "ok" };
