import {
  array,
  boolean,
  number,
  object,
  optional,
  string,
  union,
  Infer,
} from "./schema";

const userSchema = object({
  id: string(),
  age: number(),
  tags: array(string()),
  nickname: optional(string()),
  contact: object({
    email: string(),
    verified: optional(boolean()),
  }),
});

type User = Infer<typeof userSchema>;

const parsed = userSchema.parse({});
const id: string = parsed.id;
const tags: string[] = parsed.tags;
const nickname: string | undefined = parsed.nickname;
const email: string = parsed.contact.email;

const idOrCount = union(string(), number());
type IdOrCount = Infer<typeof idOrCount>;
const asText: IdOrCount = "x";
const asCount: IdOrCount = 3;

const roundTrip: User = {
  id: "1",
  age: 2,
  tags: ["a"],
  nickname: undefined,
  contact: { email: "e", verified: true },
};

const bad: number = parsed.id;

export { id, tags, nickname, email, asText, asCount, roundTrip, bad };
