type MetricQuery = { name: string; data_source: 'metrics'; query: string };
type LogQuery = { name: string; data_source: 'logs'; compute: { aggregation: string } };

type TimeseriesRequest = {
  response_format: 'timeseries';
  queries: (MetricQuery | LogQuery)[];
};

type Timeseries = { viz: 'timeseries'; requests: TimeseriesRequest[] };
type TopList = { viz: 'toplist'; requests: { q: string }[] };

declare function render(definition: Timeseries | TopList): string;

export const decidedByTheDiscriminant = render({
  viz: 'timeseries',
  requests: [
    {
      response_format: 'timeseries',
      queries: [
        { name: 'a', data_source: 'metrics', query: 'a' },
        { name: 'b', data_source: 'logs', compute: { aggregation: 'avg' } },
      ],
    },
  ],
});

type A = { kind: 'a'; n: number };
type B = { kind: 'b'; s: string };

declare function collect(input: { value: A[] } | { value: B[] }): string;

export const decidedByTheOnlyProperty = collect({
  value: [
    { kind: 'b', s: 'one' },
    { kind: 'b', s: 'two' },
  ],
});
