interface Deferred {
  promise: Promise<void>;
  resolve: () => void;
  reject: (err: unknown) => void;
}

declare function createDeferred(): Deferred;

export function createPuller(): PromiseLike<void> & {
  pull: () => void;
  reject: (err: unknown) => void;
} {
  let deferred = createDeferred();

  return {
    pull: () => {
      deferred.resolve();
    },
    reject: (err: unknown) => {
      deferred.reject(err);
    },
    then(onFulfilled, onRejected) {
      return deferred.promise.then(onFulfilled, onRejected).then((res) => {
        deferred = createDeferred();
        return res;
      });
    },
  };
}

export async function pull(): Promise<void> {
  const puller = createPuller();
  puller.pull();
  await puller;
}

export function objectSurfacesStayChecked(): { pull: () => void } {
  return { pull: () => {}, nope: 1 };
}
