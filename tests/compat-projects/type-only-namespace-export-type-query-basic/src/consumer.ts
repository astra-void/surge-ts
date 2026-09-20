import type { Utils } from './utils/index'

type Create = ReturnType<typeof Utils.RuleCreator>
type Meta = Utils.RuleMeta

export const create: Create = (id: number) => id > 0
export const wrongCreate: Create = 1
export const meta: Meta = { url: 1 }
export const asValue = Utils.RuleCreator
