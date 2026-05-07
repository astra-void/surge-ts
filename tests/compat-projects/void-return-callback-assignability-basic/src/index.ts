interface Store {
  set: (userId: string, challenge: string) => void;
}

declare const redis: {
  set(key: string, value: string): Promise<string>;
};

export const store: Store = {
  set: async (userId, challenge) => {
    await redis.set(userId, challenge);
  },
};
