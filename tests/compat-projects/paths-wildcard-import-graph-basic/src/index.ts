import { router } from "~/server/trpc";
import { missing } from "~/server/missing";

export const currentRouter = router;
export const missingRouter = missing;
