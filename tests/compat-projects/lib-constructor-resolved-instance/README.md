# lib-constructor-resolved-instance

`new Int8Array(4)` is the return type of the construct signature overload
resolution picks — `new (length: number): Int8Array<ArrayBuffer>` — so its
buffer is an `ArrayBuffer`, not assignable to a `SharedArrayBuffer`. A generic
overload infers its type argument from the buffer passed in:
`new Uint8Array(sharedBuffer)` is `Uint8Array<SharedArrayBuffer>`.
