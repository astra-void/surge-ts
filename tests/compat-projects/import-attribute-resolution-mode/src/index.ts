import type { RequireInterface } from "pkg" with { "resolution-mode": "require" };
import type { ImportInterface } from "pkg" with { "resolution-mode": "import" };
import type { RequireInterface as Unmoded } from "pkg";

export type { RequireInterface as Reexported } from "pkg" with { "resolution-mode": "require" };
export type { ImportInterface as ReexportedImport } from "pkg" with { "resolution-mode": "import" };

export interface Local extends RequireInterface, ImportInterface {}
export type Rest = [Unmoded];
