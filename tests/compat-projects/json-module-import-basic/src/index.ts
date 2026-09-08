import { version, engines, keywords, empty } from './package-info.json';
import info from './package-info.json';

export const named: string = version;
export const nested: boolean = engines.features.watch;
export const listElement: string | undefined = keywords[0];
export const emptyList: never[] = empty;

export const wholeName: string = info.name;
export const flag: boolean = info.private;
export const quotedKey: number = info['dash-key'];
export const nullable = info.missing;

export function readsThroughTheNamespace(): number {
  return info.retries;
}

export const wrongType: number = version;
export const missingKey = info.engines.nope;
