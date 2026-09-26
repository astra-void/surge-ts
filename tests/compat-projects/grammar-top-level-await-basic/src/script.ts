declare const scriptPending: Promise<number>;
declare const scriptResources: AsyncIterable<number>;
declare const scriptResource: AsyncDisposable;

await scriptPending;

for await (const item of scriptResources) {
    item;
}

{
    await using held = scriptResource;
}

async function insideScript() {
    await scriptPending;
    await using fine = scriptResource;
}
