interface Options {
  label?: string;
  count?: number | undefined;
  flag: boolean | undefined;
}

export function writes(options: Options, maybe: string | undefined) {
  options.label = undefined;
  options.count = undefined;
  options.flag = undefined;
  options.label = maybe;
}

export function presence(options: Options) {
  if ("label" in options) {
    options.label = options.label;
  } else {
    options.label = options.label;
  }
}

export function ownProperty(options: Options) {
  if (options.hasOwnProperty("label")) {
    options.label = options.label;
  }
}

export const literal: Options = { label: undefined, flag: true };
export const explicit: Options = { count: undefined, flag: undefined };

declare function configure(options: Options): void;
configure({ label: undefined, flag: false });
configure({ count: undefined, flag: false });

declare const loose: { label: string | undefined; flag: boolean };
export const copied: Options = loose;
configure(loose);

export function deletes(options: Options) {
  delete options.label;
  delete options.count;
  delete options.flag;
}
