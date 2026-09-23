# jsx-missing-intrinsic-elements-basic

With no JSX typings in the program at all, every intrinsic element is an
implicit `any` (TS7026), once per element. A closing tag (`</section>`) is
reported again; `jsx-namespace-missing-intrinsics-basic` pins that.
