"use strict";

// TypeScript's `TypeChecker`, `Type`, `Symbol` and `Signature` over the
// native checker's handles. A wrapper is created once per handle and cached
// by its program, so asking twice for the same type, symbol or signature
// gives the same object.
const { TypeFlags, ObjectFlags, SymbolFlags, SignatureKind } = require("./enums.cjs");
const { ownNode } = require("./nodes.cjs");
const { documentationOf, jsDocTagsOf } = require("./jsdoc.cjs");

class TypeObject {
	constructor(checker, id) {
		this.checker = checker;
		this._id = id;
		this._flags = undefined;
	}
	get flags() {
		if (this._flags === undefined) this._flags = this.checker._native.typeFlags(this._id);
		return this._flags;
	}
	get objectFlags() {
		return this.flags & TypeFlags.Object ? this.checker._native.objectFlags(this._id) : 0;
	}
	get symbol() {
		return this.checker._symbolOrUndefined(this.checker._native.typeSymbol(this._id));
	}
	get aliasSymbol() {
		return this.checker._symbolOrUndefined(this.checker._native.aliasSymbol(this._id));
	}
	get aliasTypeArguments() {
		const alias = this.aliasSymbol;
		const declaration = alias?.declarations?.[0];
		if (!declaration?.typeParameters?.length) return undefined;
		throw new Error("surge-ts: Type.aliasTypeArguments is not supported yet");
	}
	get value() {
		return this.flags & (TypeFlags.StringLiteral | TypeFlags.NumberLiteral) ? this.checker._native.literalValue(this._id) : undefined;
	}
	get types() {
		return this.flags & (TypeFlags.Union | TypeFlags.Intersection)
			? this.checker._native.typeConstituents(this._id).map((id) => this.checker._type(id))
			: undefined;
	}
	get intrinsicName() {
		return this.flags & TypeFlags.Intrinsic ? this.checker.typeToString(this) : undefined;
	}
	getFlags() {
		return this.flags;
	}
	getSymbol() {
		return this.symbol;
	}
	getProperties() {
		return this.checker.getPropertiesOfType(this);
	}
	getProperty(name) {
		return this.checker.getPropertyOfType(this, name);
	}
	getApparentProperties() {
		return this.checker.getPropertiesOfType(this.checker.getApparentType(this));
	}
	getCallSignatures() {
		return this.checker.getSignaturesOfType(this, SignatureKind.Call);
	}
	getConstructSignatures() {
		return this.checker.getSignaturesOfType(this, SignatureKind.Construct);
	}
	getStringIndexType() {
		return this.checker._typeOrUndefined(this.checker._native.indexTypeOfType(this._id, false));
	}
	getNumberIndexType() {
		return this.checker._typeOrUndefined(this.checker._native.indexTypeOfType(this._id, true));
	}
	getNonNullableType() {
		return this.checker.getNonNullableType(this);
	}
	getBaseTypes() {
		if (!this.isClassOrInterface()) return undefined;
		throw new Error("surge-ts: Type.getBaseTypes is not supported yet for a class or interface type");
	}
	getConstraint() {
		return this.checker.getBaseConstraintOfType(this);
	}
	getDefault() {
		if (!this.isTypeParameter()) return undefined;
		throw new Error("surge-ts: Type.getDefault is not supported yet");
	}
	isUnion() {
		return (this.flags & TypeFlags.Union) !== 0;
	}
	isIntersection() {
		return (this.flags & TypeFlags.Intersection) !== 0;
	}
	isUnionOrIntersection() {
		return (this.flags & TypeFlags.UnionOrIntersection) !== 0;
	}
	isLiteral() {
		return (this.flags & (TypeFlags.StringLiteral | TypeFlags.NumberLiteral | TypeFlags.BigIntLiteral)) !== 0;
	}
	isStringLiteral() {
		return (this.flags & TypeFlags.StringLiteral) !== 0;
	}
	isNumberLiteral() {
		return (this.flags & TypeFlags.NumberLiteral) !== 0;
	}
	isTypeParameter() {
		return (this.flags & TypeFlags.TypeParameter) !== 0;
	}
	isClassOrInterface() {
		return (this.objectFlags & ObjectFlags.ClassOrInterface) !== 0;
	}
	isClass() {
		return (this.objectFlags & ObjectFlags.Class) !== 0;
	}
	isIndexType() {
		return (this.flags & TypeFlags.Index) !== 0;
	}
}

class SymbolObject {
	constructor(checker, id) {
		this._checker = checker;
		this._id = id;
		this._info = undefined;
	}
	_details() {
		if (this._info === undefined) this._info = this._checker._native.symbolInfo(this._id);
		return this._info;
	}
	get name() {
		return this._details().name;
	}
	get escapedName() {
		return this._details().escapedName;
	}
	get flags() {
		return this._details().flags;
	}
	get declarations() {
		const declarations = this._checker._native.symbolDeclarations(this._id);
		return declarations.length ? declarations.map((reference) => this._checker._program._nodeOf(reference)) : undefined;
	}
	get valueDeclaration() {
		const reference = this._checker._native.symbolValueDeclaration(this._id);
		return reference ? this._checker._program._nodeOf(reference) : undefined;
	}
	get parent() {
		return this._checker._symbolOrUndefined(this._checker._native.symbolParent(this._id));
	}
	get members() {
		return this._table(this._checker._native.symbolMembers(this._id));
	}
	get exports() {
		return this._table(this._checker._native.symbolExports(this._id));
	}
	_table(ids) {
		if (ids.length === 0) return undefined;
		const table = new Map();
		for (const id of ids) {
			const symbol = this._checker._symbol(id);
			table.set(symbol.escapedName, symbol);
		}
		return table;
	}
	getName() {
		return this.name;
	}
	getEscapedName() {
		return this.escapedName;
	}
	getFlags() {
		return this.flags;
	}
	getDeclarations() {
		return this.declarations;
	}
	getDocumentationComment() {
		return documentationOf(this.declarations ?? []);
	}
	getJsDocTags() {
		return jsDocTagsOf(this.declarations ?? []);
	}
}

class SignatureObject {
	constructor(checker, id) {
		this._checker = checker;
		this._id = id;
	}
	get declaration() {
		const reference = this._checker._native.signatureDeclaration(this._id);
		return reference ? this._checker._program._nodeOf(reference) : undefined;
	}
	get parameters() {
		return this.getParameters();
	}
	get typeParameters() {
		const typeParameters = this.getTypeParameters();
		return typeParameters.length ? typeParameters : undefined;
	}
	getDeclaration() {
		return this.declaration;
	}
	getTypeParameters() {
		return this._checker._native.signatureTypeParameters(this._id).map((id) => this._checker._type(id));
	}
	getParameters() {
		return this._checker._native.signatureParameters(this._id).map((id) => this._checker._symbol(id));
	}
	getReturnType() {
		return this._checker.getReturnTypeOfSignature(this);
	}
	getDocumentationComment() {
		const declaration = this.declaration;
		return declaration ? documentationOf([declaration]) : [];
	}
	getJsDocTags() {
		const declaration = this.declaration;
		return declaration ? jsDocTagsOf([declaration]) : [];
	}
}

class TypeChecker {
	constructor(program) {
		this._program = program;
		this._native = program._native;
		this._types = new Map();
		this._symbols = new Map();
		this._signatures = new Map();
	}
	_type(id) {
		let type = this._types.get(id);
		if (type === undefined) {
			type = new TypeObject(this, id);
			this._types.set(id, type);
		}
		return type;
	}
	_typeOrUndefined(id) {
		return id === null || id === undefined ? undefined : this._type(id);
	}
	_symbol(id) {
		let symbol = this._symbols.get(id);
		if (symbol === undefined) {
			symbol = new SymbolObject(this, id);
			this._symbols.set(id, symbol);
		}
		return symbol;
	}
	_symbolOrUndefined(id) {
		return id === null || id === undefined ? undefined : this._symbol(id);
	}
	_signature(id) {
		let signature = this._signatures.get(id);
		if (signature === undefined) {
			signature = new SignatureObject(this, id);
			this._signatures.set(id, signature);
		}
		return signature;
	}
	_node(node) {
		ownNode(node);
		const sourceFile = node._sourceFile;
		if (sourceFile._program !== this._program) {
			throw new TypeError("surge-ts: the node belongs to another program");
		}
		return [sourceFile._fileIndex, node._index];
	}
	_typeId(type) {
		if (!(type instanceof TypeObject) || type.checker !== this) throw new TypeError("surge-ts: the type belongs to another program");
		return type._id;
	}
	_symbolId(symbol) {
		if (!(symbol instanceof SymbolObject) || symbol._checker !== this) {
			throw new TypeError("surge-ts: the symbol belongs to another program");
		}
		return symbol._id;
	}
	_signatureId(signature) {
		if (!(signature instanceof SignatureObject) || signature._checker !== this) {
			throw new TypeError("surge-ts: the signature belongs to another program");
		}
		return signature._id;
	}

	getSymbolAtLocation(node) {
		const [file, index] = this._node(node);
		return this._symbolOrUndefined(this._native.symbolAtLocation(file, index));
	}
	getTypeAtLocation(node) {
		const [file, index] = this._node(node);
		return this._type(this._native.typeAtLocation(file, index));
	}
	getTypeOfSymbolAtLocation(symbol, node) {
		const [file, index] = this._node(node);
		return this._type(this._native.typeOfSymbolAtLocation(this._symbolId(symbol), file, index));
	}
	getTypeOfSymbol(symbol) {
		return this._type(this._native.typeOfSymbol(this._symbolId(symbol)));
	}
	getDeclaredTypeOfSymbol(symbol) {
		return this._type(this._native.declaredTypeOfSymbol(this._symbolId(symbol)));
	}
	getPropertiesOfType(type) {
		return this._native.propertiesOfType(this._typeId(type)).map((id) => this._symbol(id));
	}
	getPropertyOfType(type, name) {
		return this._symbolOrUndefined(this._native.propertyOfType(this._typeId(type), name));
	}
	getSignaturesOfType(type, kind) {
		const construct = kind === SignatureKind.Construct;
		return this._native.signaturesOfType(this._typeId(type), construct).map((id) => this._signature(id));
	}
	getIndexTypeOfType(type, kind) {
		return this._typeOrUndefined(this._native.indexTypeOfType(this._typeId(type), kind === 1));
	}
	getReturnTypeOfSignature(signature) {
		return this._type(this._native.returnTypeOfSignature(this._signatureId(signature)));
	}
	getExportsOfModule(moduleSymbol) {
		return this._native.exportsOfModule(this._symbolId(moduleSymbol)).map((id) => this._symbol(id));
	}
	getAliasedSymbol(symbol) {
		return this._symbolOrUndefined(this._native.aliasedSymbol(this._symbolId(symbol))) ?? symbol;
	}
	getImmediateAliasedSymbol(symbol) {
		return this._symbolOrUndefined(this._native.aliasedSymbol(this._symbolId(symbol)));
	}
	getApparentType(type) {
		return this._type(this._native.apparentType(this._typeId(type)));
	}
	getBaseConstraintOfType(type) {
		return this._typeOrUndefined(this._native.baseConstraintOfType(this._typeId(type)));
	}
	getNonNullableType(type) {
		return this._type(this._native.nonNullableType(this._typeId(type)));
	}
	getBaseTypeOfLiteralType(type) {
		return this._type(this._native.baseTypeOfLiteralType(this._typeId(type)));
	}
	getWidenedType(type) {
		return this._type(this._native.widenedType(this._typeId(type)));
	}
	isArrayType(type) {
		return this._native.isArrayType(this._typeId(type));
	}
	isTupleType(type) {
		return this._native.isTupleType(this._typeId(type));
	}
	getTypeArguments(type) {
		return this._native.typeArguments(this._typeId(type)).map((id) => this._type(id));
	}
	isTypeAssignableTo(source, target) {
		return this._native.isTypeAssignableTo(this._typeId(source), this._typeId(target));
	}
	typeToString(type) {
		return this._native.typeToString(this._typeId(type));
	}
	symbolToString(symbol) {
		return this._native.symbolToString(this._symbolId(symbol));
	}
	signatureToString(signature) {
		return this._native.signatureToString(this._signatureId(signature));
	}
	getFullyQualifiedName(symbol) {
		const names = [];
		for (let current = symbol; current !== undefined; current = current.parent) {
			if (current.flags & SymbolFlags.ValueModule && current.name.startsWith('"')) {
				names.unshift(current.name);
				break;
			}
			names.unshift(current.name);
		}
		return names.join(".");
	}
	getAnyType() {
		return this._type(this._native.intrinsicType("any"));
	}
	getUnknownType() {
		return this._type(this._native.intrinsicType("unknown"));
	}
	getStringType() {
		return this._type(this._native.intrinsicType("string"));
	}
	getNumberType() {
		return this._type(this._native.intrinsicType("number"));
	}
	getBooleanType() {
		return this._type(this._native.intrinsicType("boolean"));
	}
	getBigIntType() {
		return this._type(this._native.intrinsicType("bigint"));
	}
	getESSymbolType() {
		return this._type(this._native.intrinsicType("symbol"));
	}
	getUndefinedType() {
		return this._type(this._native.intrinsicType("undefined"));
	}
	getNullType() {
		return this._type(this._native.intrinsicType("null"));
	}
	getVoidType() {
		return this._type(this._native.intrinsicType("void"));
	}
	getNeverType() {
		return this._type(this._native.intrinsicType("never"));
	}
	getTypeFromTypeNode(node) {
		return this.getTypeAtLocation(node);
	}
	getTypeOfPropertyOfType(type, name) {
		const property = this.getPropertyOfType(type, name);
		return property && this.getTypeOfSymbol(property);
	}
}

// Methods of TypeScript's TypeChecker surge-ts does not answer yet. Each
// throws, naming itself, rather than being absent or answering a guess.
for (const method of [
	"getAmbientModules",
	"getAugmentedPropertiesOfType",
	"getAwaitedType",
	"getBaseTypes",
	"getConstantValue",
	"getContextualType",
	"getExportSpecifierLocalTargetSymbol",
	"getExportSymbolOfSymbol",
	"getIndexInfoOfType",
	"getIndexInfosOfType",
	"getJsxIntrinsicTagNamesAt",
	"getMergedSymbol",
	"getNullableType",
	"getPromisedTypeOfPromise",
	"getResolvedSignature",
	"getRootSymbols",
	"getShorthandAssignmentValueSymbol",
	"getSignatureFromDeclaration",
	"getSymbolsInScope",
	"getTypePredicateOfSignature",
	"isArgumentsSymbol",
	"isUndefinedSymbol",
	"isUnknownSymbol",
	"isValidPropertyAccess",
	"signatureToSignatureDeclaration",
	"symbolToEntityName",
	"typeToTypeNode",
]) {
	TypeChecker.prototype[method] = function () {
		throw new Error(`surge-ts: TypeChecker.${method} is not supported yet`);
	};
}

module.exports = { TypeChecker, TypeObject, SymbolObject, SignatureObject };
