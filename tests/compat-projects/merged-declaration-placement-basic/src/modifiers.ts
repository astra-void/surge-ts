interface Mods { readonly z: boolean; y: string }
interface Mods { z: boolean; y: string }
class PrivateMods { private p: number = 1; readonly r: number = 2 }
interface PrivateMods { p: number; r: number }
interface Same { m(): void; n: string }
interface Same { m(): void; n: string }
