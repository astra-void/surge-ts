import type * as core from "./core";

interface Handler<P = core.Params, B = { id: number }> extends core.Handler<P, B> {}
interface Plain extends core.Handler<core.Params, string> {}

export const typed: Handler = (request, status) => {
  request.body.id;
  status.toFixed();
};

export const explicit: Handler<{ name: string }, boolean> = (request) => {
  request.params.name;
  request.body === true;
};

export const plain: Plain = (request) => {
  request.body.length;
};

export const wrongParameter: Plain = (request: number) => {
  request.toFixed();
};
