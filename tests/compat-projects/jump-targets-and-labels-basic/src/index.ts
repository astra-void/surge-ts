export {};

function crossesArrow() {
  for (;;) {
    const f = () => {
      break;
    };
  }
}

function crossesFunction() {
  outer: for (;;) {
    function inner() {
      continue outer;
    }
  }
}

class Holder {
  static {
    for (;;) {
      const g = function () {
        break;
      };
    }
  }
}

function continueToBlock() {
  block: {
    continue block;
  }
}

function breakToBlock() {
  block: {
    break block;
  }
}

function continueInSwitch(value: number) {
  switch (value) {
    case 1:
      continue;
  }
}

function missingLabel() {
  for (;;) {
    break nowhere;
  }
}

function duplicateLabel() {
  twice: twice: for (;;) {
    break twice;
  }
}

function nestedLoops() {
  outer: for (;;) {
    inner: while (true) {
      continue outer;
    }
  }
}

labelled: function declared() {}
