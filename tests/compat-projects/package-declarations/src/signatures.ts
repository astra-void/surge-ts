import type { User as PkgUser } from "pkg";

export function consume(user: PkgUser): string {
    return user.name;
}

export function produce(): PkgUser {
    return { id: "1", name: "test" };
}

export function badProduce(): PkgUser {
    return { id: "1", name: 123 }; // TS2322
}