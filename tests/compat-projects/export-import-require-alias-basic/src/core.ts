export import published = require("./req");
import local = require("./req");

const fromPublished = new published.Widget();
export const fromLocal: published.Widget = new local.Widget();
export const size: number = fromPublished.size;
export const missing = published.Missing;
