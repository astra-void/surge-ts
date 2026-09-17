import { I } from './dep';

interface I { y: string }

export const used: I = { y: 'ok' };
