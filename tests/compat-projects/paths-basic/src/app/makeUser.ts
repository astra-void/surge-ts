import { User } from "@models";

export function makeUser(): User {
    return { name: "test" };
}

export function getName(user: User): string {
    return user.name;
}

export function badUser(): User {
    return { name: 123 }; // TS2322
}
