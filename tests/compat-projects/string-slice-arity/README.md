# string-slice-arity

lib.es5 declares `slice(start?: number, end?: number)` but
`substring(start: number, end?: number)` and `substr(from: number, length?:
number)`, none with a rest parameter: `text.slice()` is fine,
`text.substring()` and `text.substr()` are TS2554 ("Expected 1-2
arguments"), and a third argument to `slice` is TS2554 as well.
`toLocaleLowerCase` and `toLocaleUpperCase` take optional locales, and
`localeCompare(that, locales?, options?)` its locales and collator options.
