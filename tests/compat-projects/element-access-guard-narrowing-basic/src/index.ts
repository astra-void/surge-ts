interface Post {
  title: string;
}

declare const posts: Post[];
declare const db: { posts: Post[] };

export function literalIndex(): string {
  if (posts[0]) {
    return posts[0].title;
  }
  return '';
}

export function constIndex(): string {
  const index = posts.length - 1;
  if (db.posts[index]) {
    return db.posts[index].title;
  }
  return '';
}

export function nullishTest(): string {
  const index = 2;
  if (posts[index] !== undefined) {
    return posts[index].title;
  }
  return '';
}

export function andChain(): number {
  const parts = 'a?b'.split('?') as [string, string?];
  const tail: string[] = [];
  if (parts[1] && parts[1].length > 0) {
    tail.push(parts[1]);
  }
  return tail.length;
}

export function unguardedStillReports(): string {
  return posts[0].title;
}

export function otherIndexStillReports(): string {
  if (posts[0]) {
    return posts[1].title;
  }
  return '';
}
