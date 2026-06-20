interface Lnk {
  next: Lnk;
}

function take(node: Lnk) {}

take({ next: {} });
