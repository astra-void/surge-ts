declare const pending: Promise<number>;
declare const resources: AsyncIterable<number>;
declare const resource: AsyncDisposable;

await pending;
await (pending);

for await (const item of resources) {
    item;
}

await using held = resource;

export async function inside() {
    await pending;
}
