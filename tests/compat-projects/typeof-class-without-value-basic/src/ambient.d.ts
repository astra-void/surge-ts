declare module "ambient-lib" {
  class Message {
    url: string;
  }
  type Listener<R extends typeof Message = typeof Message> = (
    request: InstanceType<R>,
  ) => void;
  export { Message, Listener };
}
