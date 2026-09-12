interface Router<TRecord> {
  _def: {
    record: TRecord;
  };
}

declare function router<T extends Record<string, unknown>>(value: T): Router<T> & T;

type Procedure<T> = {
  query(): Promise<T>;
};

type Decorate<T> = {
  [K in keyof T]: T[K] extends Procedure<infer _R>
    ? T[K]
    : T[K] extends Record<string, unknown>
      ? Decorate<T[K]>
      : never;
};

type Client<TRouter extends Router<any>> = Decorate<TRouter['_def']['record']>;

declare function createClient<T extends Router<any>>(): Client<T>;

interface Post {
  title: string;
}

const appRouter = router({
  post: {
    listPosts: {} as Procedure<Post[]>,
  },
});

type AppRouter = typeof appRouter;

const client = createClient<AppRouter>();

export async function f() {
  const posts = await client.post.listPosts.query();
  posts[0].title;
}
