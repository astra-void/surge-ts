export function RuleCreator(url: (name: string) => string): (id: number) => boolean {
  return () => url('x') === 'x'
}
export type RuleMeta = { url: string }
