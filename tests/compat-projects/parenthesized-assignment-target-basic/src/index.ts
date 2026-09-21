declare let count: number;
declare const box: { inner: { value: number } };
const fixed = 1;

count = "";
(count) = "";
((count)) = "";
box.inner.value = "";
(box.inner).value = "";
(box.inner.value) = "";
(fixed) = 2;

export function inFunction(local: number) {
  (local) = "";
  (box.inner.value) = "";
  return local;
}
