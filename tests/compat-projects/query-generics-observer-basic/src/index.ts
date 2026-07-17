import { useQuery, QueryResult } from "query-core";

interface User {
  id: string;
  name: string;
}

const users = useQuery<User[], Error>({
  queryKey: ["users"],
  queryFn: () => Promise.resolve<User[]>([{ id: "1", name: "a" }]),
  onSuccess: (data) => {
    const first: User | undefined = data[0];
    void first;
  },
  select: (data) => data.filter((user) => user.name.length > 0),
});

const inferred = useQuery({
  queryKey: ["users", "inferred"],
  queryFn: () => Promise.resolve<User[]>([]),
});

const again: QueryResult<User[], Error> = useQuery<User[], Error>({
  queryKey: ["users", "again"],
  queryFn: () => Promise.resolve<User[]>([]),
});

const names: string[] | undefined = users.data?.map((user) => user.name);
const message: string | undefined = users.error?.message;
const bad: User[] = users.data;
const badCallback = useQuery<number, Error>({
  queryKey: ["count"],
  queryFn: () => Promise.resolve(1),
  onSuccess: (data: string) => {
    void data;
  },
});
void inferred;
void again;
void names;
void message;
void bad;
void badCallback;
