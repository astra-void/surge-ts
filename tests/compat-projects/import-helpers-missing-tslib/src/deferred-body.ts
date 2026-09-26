export const pick = (input: { a: number; b: number }) => {
  const { a, ...rest } = input;
  return rest;
};

export async function load() {
  return pick({ a: 1, b: 2 });
}
