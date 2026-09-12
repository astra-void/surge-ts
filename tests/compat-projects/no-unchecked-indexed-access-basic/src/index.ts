interface Post {
  title: string;
}

declare const posts: Post[];
declare const pair: [Post, Post];
declare const record: Record<string, Post>;
declare const sized: { [key: string]: Post; first: Post };
declare function fetchPosts(): Promise<Post[]>;

export const arrayElement = posts[0].title;
export const arrayVariable = posts[posts.length - 1].title;
export const tupleElementIsExact = pair[0].title;
export const recordKey = record['a'].title;
export const recordDot = record.a.title;
export const declaredMember = sized.first.title;
export const indexMember = sized.other.title;

export async function awaited(): Promise<string> {
  const list = await fetchPosts();
  return list[0].title;
}

export function destructured(): string {
  const [head] = posts;
  return head.title;
}

export function guarded(): string {
  const head = posts[0];
  if (!head) {
    return '';
  }
  return head.title;
}

export function iterated(): string[] {
  const titles: string[] = [];
  for (const post of posts) {
    titles.push(post.title);
  }
  return titles;
}

export function assigned(): void {
  posts[0] = { title: 'x' };
  record['b'] = { title: 'y' };
}
