# constructor-overload-group-basic

A class's construct signatures are all of its constructor overloads (the
implementation is hidden behind them outside an ambient class). surge took
only the first constructor declaration, so `new WebSocket(url)` against
`@types/ws`'s `constructor(address: null)` / `constructor(address: string | URL)`
reported the string argument; an argument no overload accepts is still TS2769.
