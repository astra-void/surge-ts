declare module "namespace-first" {
  namespace nsFirst {
    interface PlatformPath {
      resolve(...paths: string[]): string;
    }
  }
  const nsFirst: nsFirst.PlatformPath;
  export = nsFirst;
}

declare module "value-first" {
  const valueFirst: valueFirst.PlatformPath;
  namespace valueFirst {
    interface PlatformPath {
      resolve(...paths: string[]): string;
    }
  }
  export = valueFirst;
}

declare module "aliased-namespace-first" {
  import nsFirst = require("namespace-first");
  export = nsFirst;
}

declare module "function-namespace-merge" {
  function fnMerge(): void;
  namespace fnMerge {
    const version: string;
  }
  export = fnMerge;
}
