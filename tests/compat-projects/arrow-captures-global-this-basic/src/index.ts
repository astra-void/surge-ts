const f = () => this;
const g = () => { return () => this; };
function h() { return () => this; }
const top = this;
class C { m() { return () => this; } p = () => this; }
const o = { m() { return () => this; } };
