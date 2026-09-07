import * as paths from './paths';

export const real = paths.realpathSync('/tmp');
export const native = paths.realpathSync.native('/tmp');
export const separator = paths.realpathSync.separator;

export const stillChecked: number = paths.realpathSync('/tmp');
