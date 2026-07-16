interface Success<TData> {
  status: "success";
  data: TData;
}

interface Failure<TError> {
  status: "error";
  error: TError;
}

type Result<TData, TError> = Success<TData> | Failure<TError>;

interface Observer<TData, TError> {
  getCurrentResult(): Result<TData, TError>;
  subscribe(listener: (value: Result<TData, TError>) => void): () => void;
}

declare const observer: Observer<string, Error>;

for (let index = 0; index < 200; index += 1) {
  observer.subscribe((value) => {
    if (value.status === "success") {
      const data: string = value.data;
      void data;
    }
  });
}
