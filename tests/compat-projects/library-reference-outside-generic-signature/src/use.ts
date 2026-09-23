import type { Api } from 'lib';

export function transform(j: Api) {
    j("").find(j.Declaration).forEach((node) => {
        const first: number = node.declarations[0];
    });
}
