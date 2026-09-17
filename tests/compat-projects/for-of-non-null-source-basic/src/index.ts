declare const maybe: number[] | undefined;
declare const opaque: unknown;
declare const holder: { list?: string[] };

for (const x of maybe) {
  void x;
}
for (const x of opaque) {
  void x;
}
for (const x of holder.list) {
  void x;
}
for (const x of null) {
  void x;
}

export async function drain(source: AsyncIterable<number> | undefined) {
  for await (const x of source) {
    void x;
  }
}
