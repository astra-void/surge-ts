const t: [string] = { k: 1 }; declare function f<T>(v: T): void; f<[string]>({ k: 1 });
