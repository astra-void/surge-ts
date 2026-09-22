class Builder<TContext extends object, TMeta extends object> {
  context<TNewContext extends object>() {
    return new Builder<TNewContext, TMeta>();
  }
  meta<TNewMeta extends object>() {
    return new Builder<TContext, TNewMeta>();
  }
  create(): { ctx: TContext; meta: TMeta } {
    return null as never;
  }
}

const init = new Builder<object, object>();
const root = init.context<{ user: string }>().meta<{ role: 'admin' }>().create();

export const user: number = root.ctx.user;
export const role: 'user' = root.meta.role;
export const missing = root.ctx.nope;
