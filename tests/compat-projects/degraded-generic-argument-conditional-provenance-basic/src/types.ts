export interface PageContext {
  pathname: string;
}

export interface RouterBase {
  withTRPC: (context: PageContext) => void;
}

export interface RouterRecord {
  router99: () => string;
}
