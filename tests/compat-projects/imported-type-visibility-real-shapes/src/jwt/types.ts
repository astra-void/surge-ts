export interface JWTPayload {
  sub: string;
  exp: number;
}

export interface JWTOptions {
  payload: JWTPayload;
}
