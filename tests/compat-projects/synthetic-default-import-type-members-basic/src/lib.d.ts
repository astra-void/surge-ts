declare module "lib" {
  namespace lib {
    interface Options {
      port: number;
    }
    class Server<R = Options> {
      options: R;
    }
    type Listener<S extends typeof Server = typeof Server> = (server: InstanceType<S>) => void;
    function create(options: Options): Server;
  }
  export = lib;
}
