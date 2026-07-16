function transform(bytes: Uint8Array) {
  const mapped = bytes.map((value) => value + 1);
  const entries = Array.from(mapped.entries());
  return { mapped, entries, first: mapped.at(0) };
}

const consumers = Array.from({ length: 200 }, () => transform(new Uint8Array(32)));
const first: number | undefined = consumers[0]?.first;
void first;
