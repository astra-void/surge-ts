import { array, lazy, object, string, Schema } from "./schema";

interface Category {
  name: string;
  children: Category[];
}

const categorySchema: Schema<Category> = lazy<Category>(() =>
  object({
    name: string(),
    children: array(categorySchema),
  }),
);

const parsed = categorySchema.parse({});
const name: string = parsed.name;
const depth2: string = parsed.children[0].children[0].name;

const bad: string = parsed.children;
void name;
void depth2;
void bad;

export { categorySchema };
