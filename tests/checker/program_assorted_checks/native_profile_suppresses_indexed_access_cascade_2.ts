// @surge-args: --diagnosticProfile native
// @surge-expect: TS2304

        interface User { name: string; }
        type UnresolvedKeyIndex = User[MissingKeyName];
        let _trigger: UnresolvedKeyIndex;
        
