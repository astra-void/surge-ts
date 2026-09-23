# index-signature-key-slots-basic

Every constituent of an index signature's key type declares its own index
info, and a `symbol` key declares one that answers no string or number key
(tsc's `getIndexInfosOfIndexSymbol`). A `[key: symbol]: never` member used to
land in the string index slot and overwrite the real one, so a JSON-shaped
type like remix/trpc's `JsonObject` refused every object — and `Serialize<T>`
never took its `IsJson` branch for a procedure output.
