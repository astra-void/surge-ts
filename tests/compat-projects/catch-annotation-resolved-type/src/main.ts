type AnyAlias = any;
type UnknownAlias = unknown;
try { } catch (a: AnyAlias) { a.anything; }
try { } catch (u: UnknownAlias) { console.log(u); }
try { } catch (n: number) { n.toLowerCase(); }
try { } catch (o: object) { }
export {};
