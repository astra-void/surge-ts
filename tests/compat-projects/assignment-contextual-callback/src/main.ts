export function reassign(handler?: (value: number) => void) {
  handler = (value) => value.toFixed();
  handler &&= (value) => value.toFixed();
  let local: ((text: string) => number) | undefined;
  local = (text) => text.length;
  local ||= (text) => text.length;
  let loose;
  loose = (x) => x;
  let typedAny: any;
  typedAny = (y) => y;
  return [handler, local, loose, typedAny];
}
