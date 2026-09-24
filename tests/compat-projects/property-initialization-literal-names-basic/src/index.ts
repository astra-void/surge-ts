const key = "k";

export class Checked {
    plain: string;
    ["computed"]: string;
    [key]: string;
    accessor auto: string;
    optional?: string;
    constructor() {}
}

export class Unchecked {
    "quoted": string;
    "1": number;
    0x10: boolean;
    2.5: string;
}
