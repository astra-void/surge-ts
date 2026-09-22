# heritage-await-globalthis-basic

Pins three checks that had no fixture: a class that `extends` an interface
(TS2689), `await` in a function that is not `async` (TS1308), and a member read
off `globalThis` that no declaration provides, which is an implicit `any`
(TS7017).
