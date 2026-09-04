import { db } from "definitely-missing-package";

export function inReturn() {
  return db.findMany({ orderBy: (fields, ops) => ops.desc(fields) });
}

export function inConst() {
  const rows = db.findMany({ orderBy: (fields, ops) => ops.desc(fields) });
  return rows;
}

export async function inAwait() {
  const rows = await db.query.Post.findMany({
    where: (fields, ops) => ops.eq(fields.id, 1),
  });
  return rows;
}
