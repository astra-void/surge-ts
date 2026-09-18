declare const xs: number[];

export function total(): void {
  for (const k in xs) {
    void xs[k];
  }
}
