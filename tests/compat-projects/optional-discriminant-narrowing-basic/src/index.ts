type Tagged = { tag?: "only"; value: number };

export function afterReturn(tagged: Tagged): number {
  if (tagged.tag === "only") return tagged.value;
  const unreachable: never = tagged;
  const asText: string = tagged;
  const tagAfter: string = tagged.tag;
  const value: number = tagged.value;
  return [unreachable, asText, tagAfter, value].length;
}

export function insideNotEqual(tagged: Tagged): void {
  if (tagged.tag !== "only") {
    const asText: string = tagged;
    void asText;
  } else {
    const tag: "only" = tagged.tag;
    void tag;
  }
}

type Mixed = { tag?: "a"; x: number } | { tag: "b"; y: number };

export function unionComplement(mixed: Mixed): number {
  if (mixed.tag === "b") return mixed.y;
  const onlyA: { tag?: "a"; x: number } = mixed;
  return onlyA.x;
}

export function unionKeepsOptional(mixed: Mixed): number {
  if (mixed.tag === "a") return mixed.x;
  const onlyB: { tag: "b"; y: number } = mixed;
  return onlyB.y;
}
