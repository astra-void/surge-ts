declare function accept(fn: (opts: unknown) => unknown): unknown;
declare function acceptTyped(fn: (opts: { ctx: { user: string }; input: string }) => unknown): unknown;

function a({ id }) {
  return id;
}

function b({ id, name }) {
  return id + name;
}

function c({ id: userId }) {
  return userId;
}

function d({ id }: { id: string }) {
  return id;
}

function e(user) {
  return user;
}

const f = ({ ctx, input }) => input;
const g = ({ ctx, input }: { ctx: { user: string }; input: string }) => input;

accept(({ ctx, input }) => input);
acceptTyped(({ ctx, input }: { ctx: { user: string }; input: string }) => input);
