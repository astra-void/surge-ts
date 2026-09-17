# member-kind-override-basic

tsc's `checkKindsOfPropertyMemberOverrides`: an instance member may not
change kind from its base class — a property overriding an accessor
(TS2610), an accessor overriding a property (TS2611) or method (TS2423), a
method overriding a property (TS2425) or accessor (TS2426). Private members
on either side and abstract base properties are exempt, and the message
names the direct base. surge reported none; it now checks bases declared in
the same file.
