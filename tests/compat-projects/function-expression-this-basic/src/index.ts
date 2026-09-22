export {};

const plain = function () {
  return this;
};
const member = {
  m: function () {
    return this;
  },
};
declare const target: { m: () => void };
target.m = function () {
  this;
};
(function () {
  this;
})();
const typed: () => void = function () {
  this;
};
const typedThis: (this: string) => void = function () {
  this.length;
};
function outer() {
  return function () {
    return this;
  };
}
const list = [
  function () {
    this;
  },
];
const choice = true
  ? function () {
      this;
    }
  : null;
