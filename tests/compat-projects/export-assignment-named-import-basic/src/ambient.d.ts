declare module 'classmod' {
  class Foo {
    static stat(): void;
  }
  namespace Foo {
    interface Opts {
      a: number;
    }
  }
  export = Foo;
}

declare module 'nsmod' {
  namespace ns {
    class Bar {}
    function helper(): void;
    interface Baz {
      z: number;
    }
  }
  export = ns;
}

declare module 'aliasmod' {
  import target = require('classmod');
  export = target;
}
