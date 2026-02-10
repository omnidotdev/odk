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
export type KnownExtension =
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

export interface CompiledSchema {
  name: string;
  models: CompiledModel[];
  unions: CompiledUnion[];
  /** Raw TypeSpec blocks to pass through verbatim */
  rawBlocks?: RawTypeSpecBlock[];
  /** TypeSpec imports required by this schema */
  imports?: string[];
  warnings?: string[];
}

// ============================================================
// Helper Functions
// ============================================================

/** Create an empty extensions object. */
export function createExtensions(): Extensions {
  return { known: [] };
}

/** Add a decorator to extensions. */
export function addDecorator(
  extensions: Extensions,
  decorator: string,
): Extensions {
  const known = extensions.known ?? [];
  return {
    ...extensions,
    known: [...known, { kind: "decorator", value: decorator }],
  };
}

/** Get all decorators from extensions. */
export function getDecorators(extensions: Extensions): string[] {
  return (extensions.known ?? [])
    .filter(
      (ext): ext is { kind: "decorator"; value: string } =>
        ext.kind === "decorator",
    )
    .map((ext) => ext.value);
}

/**
 * Serialize a CompiledSchema to JSON for the Rust CLI.
 */
export function serializeSchema(schema: CompiledSchema): string {
  return JSON.stringify(schema, null, 2);
}
