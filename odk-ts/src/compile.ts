/**
 * CompiledSchema: The intermediate representation (IR) for ODK schemas.
 * This format is consumed by the Rust CLI (tspgen) to emit TypeSpec.
 */

// ============================================================
// Source Mapping Types
// ============================================================

/** A span in a source file for error mapping. */
export interface SourceSpan {
  file: string;
  line: number;
  column: number;
  endLine?: number;
  endColumn?: number;
}

/** A raw TypeSpec block to pass through verbatim. */
export interface RawTypeSpecBlock {
  content: string;
  source?: SourceSpan;
  description?: string;
}

// ============================================================
// Extension Types
// ============================================================

/** Known extension types that ODK understands. */
type KnownExtension =
  | { kind: "zodTransform"; code: string }
  | { kind: "zodRefinement"; code: string; message?: string }
  | { kind: "decorator"; value: string }
  | { kind: "openApi"; key: string; value: unknown }
  | { kind: "drizzle"; decorator: string; args?: string[] };

/** Container for extensions on a field or model. */
export interface Extensions {
  known?: KnownExtension[];
  [key: string]: unknown; // Unknown extensions pass through
}

// ============================================================
// Core Schema Types
// ============================================================

export interface CompiledField {
  name: string;
  type: CompiledType;
  optional?: boolean;
  description?: string;
  constraints?: FieldConstraints;
  /** Decorators and extensions for this field */
  extensions?: Extensions;
  /** Source location for error mapping */
  source?: SourceSpan;
}

export interface FieldConstraints {
  minLength?: number;
  maxLength?: number;
  minValue?: number;
  maxValue?: number;
  pattern?: string;
  format?: string;
  multipleOf?: number;
}

export type CompiledType =
  | { kind: "scalar"; name: ScalarType }
  | { kind: "array"; items: CompiledType }
  | { kind: "ref"; name: string }
  | { kind: "literal"; value: string | number | boolean }
  | { kind: "union"; variants: CompiledType[] }
  | { kind: "object"; fields: CompiledField[] };

export type ScalarType =
  | "string"
  | "int32"
  | "int64"
  | "float32"
  | "float64"
  | "boolean"
  | "date"
  | "datetime"
  | "bytes"
  | "unknown";

export interface CompiledModel {
  name: string;
  fields: CompiledField[];
  description?: string;
  /** Decorators and extensions for this model */
  extensions?: Extensions;
  /** Source location for error mapping */
  source?: SourceSpan;
}

export interface CompiledUnion {
  name: string;
  variants: CompiledUnionVariant[];
  description?: string;
  /** Decorators and extensions for this union */
  extensions?: Extensions;
  /** Source location for error mapping */
  source?: SourceSpan;
}

export interface CompiledUnionVariant {
  name: string;
  type?: CompiledType;
}

/** The kind of operation. */
export type OperationKind =
  | "query"
  | "mutation"
  | "subscription"
  | "rpc"
  | "endpoint";

/** A compiled operation representing something a client can do. */
export interface CompiledOperation {
  name: string;
  kind: OperationKind;
  input?: CompiledType;
  output: CompiledType;
  selection?: SelectionSet;
  description?: string;
  extensions?: Extensions;
}

/** A set of selected fields within an operation. */
export interface SelectionSet {
  fields: FieldSelection[];
}

/** A single field selection within a selection set. */
export interface FieldSelection {
  name: string;
  alias?: string;
  arguments?: [string, CompiledType][];
  selection?: SelectionSet;
  directives?: Directive[];
}

/** A directive applied to a field selection. */
export interface Directive {
  name: string;
  arguments?: [string, unknown][];
}

export interface CompiledSchema {
  name: string;
  models: CompiledModel[];
  unions: CompiledUnion[];
  /** Operation definitions (queries, mutations, endpoints, RPCs) */
  operations?: CompiledOperation[];
  /** Raw TypeSpec blocks to pass through verbatim */
  rawBlocks?: RawTypeSpecBlock[];
  /** TypeSpec imports required by this schema */
  imports?: string[];
  warnings?: string[];
}

/**
 * Serialize a CompiledSchema to JSON for the Rust CLI.
 */
export function serializeSchema(schema: CompiledSchema): string {
  return JSON.stringify(schema, null, 2);
}
