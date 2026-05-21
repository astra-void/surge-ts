export interface JWTPayload {
  sub: string;
  exp: number;
}

export interface JWTOptions {
  payload: JWTPayload;
}

const ok: JWTOptions = { payload: { sub: "u1", exp: 1 } };
const wrong: JWTOptions = { payload: { sub: "u1", exp: "bad" } };
