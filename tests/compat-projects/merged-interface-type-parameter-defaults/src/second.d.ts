declare namespace NS {
    interface Iter<T, TReturn = undefined, TNext = any> { second: T }
}
interface Holder<T = number> { also: T }
interface Checked<T extends string = "c"> { again: T }
