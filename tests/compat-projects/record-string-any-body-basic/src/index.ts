export type Body = Record<string, any>;

export interface Provider {
  authorize: (body: Body) => Promise<unknown>;
}

export const body: Body = { email: "a", password: "b" };
body.email;
body["password"];
