# never-call-statement-exits

tsc ends the flow at a call statement whose callee is declared to return
`never` (`isReachableFlowNode` on a `FlowCall`) — a function, a parameter or a
`this` method alike, and wherever the statement sits, not only as a body's
last statement. So a function that reaches such a call has no reachable end
(no TS2366), and the fall-through of an `if` whose branch ends in one is
narrowed. Only `falls` reaches its end.
