export interface Params {
  [key: string]: string;
}
export interface Request<P = Params, B = unknown> {
  params: P;
  body: B;
}
export type Handler<P = Params, B = unknown> = (request: Request<P, B>, status: number) => void;
