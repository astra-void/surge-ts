interface Marker {}
interface Other {}
export class HeritageA extends Object extends Object {}
export class HeritageB implements Marker implements Other {}
export class HeritageC implements Marker extends Object {}
export class HeritageD extends Object, Object {}
