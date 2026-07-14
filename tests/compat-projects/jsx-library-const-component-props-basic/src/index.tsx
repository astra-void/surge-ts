import * as BoxPrimitive from "@ui/box";
import type * as React from "react";

function Box({ ...props }: React.ComponentProps<typeof BoxPrimitive.Root>) {
    return <BoxPrimitive.Root {...props} />;
}

const el = <Box onCheckedChange={(checked) => { const flag: boolean = checked; }} />;

export { el };
