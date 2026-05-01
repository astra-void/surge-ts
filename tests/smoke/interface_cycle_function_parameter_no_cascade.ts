interface Node {
  next: Node;
}

function take(node: Node) {}

take({ next: {} });
