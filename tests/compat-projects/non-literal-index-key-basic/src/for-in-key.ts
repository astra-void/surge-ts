const o = { a: 1, b: 2 };

export function total(): void {
  for (const k in o) {
    void o[k];
  }
}
