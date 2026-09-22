declare const text: string;
export const first: number = text[0];

const filters = ['all', 'active', 'completed'] as const;
export const capitalised = filters.map(
  (filter) => filter[0].toUpperCase() + filter.slice(1),
);

declare const rows: { title: string }[];
export const title: string = rows[0].title;
