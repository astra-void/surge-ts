
        interface User { name: string; }
        type UnresolvedKeyIndex = User[MissingKeyName];
        let _trigger: UnresolvedKeyIndex;
        
