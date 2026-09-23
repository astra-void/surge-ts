class Holder {
  get held() {
    return null;
  }
  set held(value) {}
  field = null;
  list = [null];
  empty = [];
}
declare const holder: Holder;
holder.held = 1;
holder.field = "text";
holder.list = [1];
holder.empty = ["a"];

let pair = [null, undefined];
pair = [1, 2];
let nested = { values: [null] };
nested.values = [1];
let kept: string = null;
kept = undefined;
