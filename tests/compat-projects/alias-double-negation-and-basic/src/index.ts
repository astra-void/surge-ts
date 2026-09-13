interface Query {
  isFetched(): boolean;
  setState(state: object): void;
  resetState: object;
}

declare function find(): Query | undefined;

export function inline(mode: 'reset' | 'append'): void {
  const query = find();
  if (!!query && mode === 'reset') {
    query.setState({ ...query.resetState });
  }
}

export function aliased(mode: 'reset' | 'append'): void {
  const query = find();
  const isRefetch = !!query && query.isFetched();
  if (isRefetch && mode === 'reset') {
    query.setState({ ...query.resetState });
  }
}

export function negated(mode: 'reset' | 'append'): void {
  const query = find();
  if (!query && mode === 'reset') {
    // @ts-expect-error `!query` proves the opposite: query is undefined here
    query.setState({});
  }
}
