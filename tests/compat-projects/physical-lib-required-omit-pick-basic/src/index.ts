// Mirrors ky's `InternalRetryOptions`. `Required<Omit<…>>` must make each
// picked property required while keeping an explicit `| undefined` member
// (so `jitter: undefined` is valid under exactOptionalPropertyTypes).
type RetryOptions = {
  limit?: number;
  methods?: string[];
  jitter?: boolean | ((attempt: number) => number) | undefined;
  shouldRetry?: () => boolean;
};

type InternalRetryOptions = Required<Omit<RetryOptions, 'shouldRetry'>> &
  Pick<RetryOptions, 'shouldRetry'>;

export const defaultRetryOptions: InternalRetryOptions = {
  limit: 2,
  methods: ['get', 'put'],
  jitter: undefined,
};
