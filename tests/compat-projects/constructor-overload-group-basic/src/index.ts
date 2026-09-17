declare class Socket {
  constructor(address: null);
  constructor(address: string | number, options?: {});
}

declare const port: number;
export const a = new Socket(`ws://localhost:${port}/ws`);
export const b = new Socket("ws://x", {});
export const c = new Socket(null);

export class Point {
  constructor(x: string);
  constructor(x: number);
  constructor(x: any) {
    void x;
  }
}

export const p1 = new Point("x");
export const p2 = new Point(1);
export const p3 = new Point(true);
