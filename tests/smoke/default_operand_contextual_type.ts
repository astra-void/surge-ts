interface Params {
  uri?: (id: string) => string;
  cb?: (id: string) => string;
}

export function nullish(params: Params): string {
  const generate = params.uri ?? ((id) => id);
  return generate("x");
}

export function logicalOr(params: Params): string {
  const generate = params.cb || ((id) => id);
  return generate("x");
}
