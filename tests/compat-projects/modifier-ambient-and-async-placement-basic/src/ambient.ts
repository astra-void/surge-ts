declare async function f(): Promise<void>;
async declare function g(): Promise<void>;
declare namespace M {
  async function h(): Promise<void>;
}
class C {
  override declare y: number;
}
declare function ok(): Promise<void>;
async function ok2(): Promise<void> {}
export {};
