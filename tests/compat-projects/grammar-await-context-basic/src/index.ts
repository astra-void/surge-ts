declare const pending: Promise<number>;

export async function fields() {
    class Fields {
        [await pending] = 0;
        value = await pending;
        static shared = await pending;
        accessor tracked = await pending;
        callback = async () => await pending;
    }
    enum Codes {
        First = await pending,
    }
    return [Fields, Codes];
}

export class TopLevel {
    value = await pending;
}

export function plain() {
    class Holder {
        static {
            await pending;
        }
    }
    return Holder;
}
