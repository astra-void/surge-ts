function orNull(value = null) {
  return value;
}
function orUndefined(value = undefined) {
  return value;
}
function leading(value = null, next: string) {
  return next;
}

orNull("text");
orNull(null);
orNull();
orUndefined(1);
orUndefined();
leading(1, "a");
leading(undefined, "a");

let later = null;
later = "text";
