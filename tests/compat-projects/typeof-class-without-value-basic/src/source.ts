export class Payload {
  size = 0;
}
export type Handler<P extends typeof Payload = typeof Payload> = (
  payload: InstanceType<P>,
) => number;
