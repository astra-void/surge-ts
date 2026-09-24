# private-name-placement-basic

TS18016 for a private name outside a class body: `#x in o` with no enclosing
class (`checkGrammarPrivateIdentifierExpression`), a private key in an object
literal (`checkGrammarObjectLiteralExpression`), and a private property or
method signature (`checkPropertySignature`/`checkSignatureDeclaration`).
Inside a class every form is fine.
