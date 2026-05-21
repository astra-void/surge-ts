interface ChallengeStore {
  get?: (key: string) => Promise<string | undefined>;
}

export function makeStore(redis: any): ChallengeStore {
  return {
    get: async (key) => {
      const result = await redis.get(key);
      return result;
    },
  };
}

export function badStore(): ChallengeStore {
  return {
    get: async (_key) => {
      return 123;
    },
  };
}
