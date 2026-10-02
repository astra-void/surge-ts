const wrap = <K extends [string, Record<string, unknown>?]>(
k: K,
fetcher: (o: K[1]) => void,
) => [k, fetcher];
export const r = wrap([""], () => {});
