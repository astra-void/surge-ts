interface Tag {
  (strings: TemplateStringsArray, ...rest: boolean[]): Tag;
  member: Tag;
  [index: number]: Tag;
}
declare const tag: Tag;

tag`a${1}b${2}`;
tag`a${true}`.member`b${1}`;
tag`a`[0].member`b${1}`;
tag[0].member`b${1}`;
export const viaIndex: number = tag[0].member;

interface Keyed {
  inner: { count: number };
  [key: string]: { count: number };
}
declare const keyed: Keyed;
declare const key: string;
export const literalKey: string = keyed["inner"].count;
export const computedKey: string = keyed[key].count;

type Result<T> = [undefined, T] | [Error, undefined];
interface Ok {
  data: number;
}
declare const named: Result<Ok>;
declare function load(): Result<Ok>;
declare const pending: Promise<Result<Ok>>[];

export function fromNamed() {
  const [error, result] = named;
  const viaTernary = error ? 0 : result.data;
  const viaAnd = !error && result.data;
  const viaOr = error || result.data;
  const inClosure = () => {
    if (error) {
      return 0;
    }
    return result.data;
  };
  const shadowed = (error: boolean) => (error ? 0 : result.data);
  return [viaTernary, viaAnd, viaOr, inClosure, shadowed];
}

export function fromCall() {
  const [error, result] = load();
  if (error) {
    return 0;
  }
  const narrowed: string = result;
  return narrowed;
}

export async function fromAwait() {
  const [error, result] = await pending[0]!;
  const viaTernary = error ? 0 : result.data;
  const inClosure = () => {
    if (error) {
      return 0;
    }
    return result.data;
  };
  return [viaTernary, inClosure];
}
