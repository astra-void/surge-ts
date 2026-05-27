function id<T>(value: T): T {
  return value;
}

let value: string = id<string>("hello");
