declare type ID = string;

declare interface User {
  name: string;
}

declare const console: {
  log: (message: string) => void;
};

declare function setTimeout(callback: unknown, ms: number): number;
declare let process: unknown;
