/**
 * ODK - Omni Development Kit
 *
 * Main API for defining schemas and emitting to various targets.
 *
 * @example
 * ```typescript
 * import { odk } from "@omnidotdev/odk";
 * import { z } from "zod";
 *
 * const User = z.object({
 *   id: z.string().uuid(),
 *   email: z.string().email(),
 *   name: z.string().optional(),
 * });
 *
 * const schema = odk.schema("MyAPI", { User });
 *
 * await odk.emit(schema, {
 *   targets: ["@typespec/openapi3"],
 *   outDir: "./generated",
 * });
 * ```
 *
 * @example Escape hatches for advanced use cases
 * ```typescript
 * // Add raw TypeSpec code
 * const schema = odk.schema("MyAPI", { User });
 * odk.raw(schema, `
 *   @route("/api")
 *   interface CustomApi {
 *     @get op health(): void;
 *   }
 * `);
 *
 * // Add custom imports
 * odk.addImport(schema, "@typespec/http");
 * ```
 */

import { standardSchemaToCompiled } from "./standardSchemaAdapter.js";
import { compiledSchemaToTypeSpec, emitToTargets } from "./typespecBridge.js";

import type { CompiledSchema, RawTypeSpecBlock } from "./compile.js";
import type { AdapterOptions } from "./standardSchemaAdapter.js";
import type { EmitOptions } from "./typespecBridge.js";

export interface SchemaOptions extends AdapterOptions {
  /** Description for the schema namespace */
  description?: string;
  /** TypeSpec imports to include */
  imports?: string[];
}

export interface ODKSchema {
  /** The compiled schema representation */
  compiled: CompiledSchema;
  /** The original types passed to schema() */
  types: Record<string, unknown>;
}

/**
 * Create an ODK schema from Standard Schema-compliant types.
 *
 * @param name - Name for the schema namespace
 * @param types - Record of named types (Zod, ArkType, or Valibot schemas)
 * @param options - Optional configuration
 * @returns ODK schema object
 *
 * @example
 * ```typescript
 * const User = z.object({ id: z.string(), name: z.string() });
 * const Post = z.object({ title: z.string(), authorId: z.string() });
 *
 * const schema = odk.schema("Blog", { User, Post });
 * ```
 */
function schema(
  name: string,
  types: Record<string, unknown>,
  options: SchemaOptions = {},
): ODKSchema {
  const compiled = standardSchemaToCompiled(name, types, options);

  // Add imports from options
  if (options.imports) {
    compiled.imports = [...(compiled.imports ?? []), ...options.imports];
  }

  return { compiled, types };
}

/**
 * Add a raw TypeSpec block to the schema.
 *
 * Use this escape hatch when you need to write TypeSpec code that
 * ODK's schema API doesn't support.
 *
 * @param schema - ODK schema to modify
 * @param content - Raw TypeSpec code
 * @param description - Optional description/comment
 *
 * @example
 * ```typescript
 * odk.raw(schema, `
 *   @route("/api")
 *   interface CustomApi {
 *     @get op health(): void;
 *   }
 * `);
 * ```
 */
function raw(schema: ODKSchema, content: string, description?: string): void {
  const block: RawTypeSpecBlock = { content };
  if (description) {
    block.description = description;
  }

  if (!schema.compiled.rawBlocks) {
    schema.compiled.rawBlocks = [];
  }
  schema.compiled.rawBlocks.push(block);
}

/**
 * Add a TypeSpec import to the schema.
 *
 * @param schema - ODK schema to modify
 * @param importPath - Import path (e.g., "@typespec/http")
 *
 * @example
 * ```typescript
 * odk.addImport(schema, "@typespec/http");
 * odk.addImport(schema, "@omnidotdev/typespec-drizzle");
 * ```
 */
function addImport(schema: ODKSchema, importPath: string): void {
  if (!schema.compiled.imports) {
    schema.compiled.imports = [];
  }
  if (!schema.compiled.imports.includes(importPath)) {
    schema.compiled.imports.push(importPath);
  }
}

/**
 * Emit an ODK schema to target formats.
 *
 * @param schema - ODK schema created with odk.schema()
 * @param options - Emission options (targets, output directory)
 *
 * @example
 * ```typescript
 * await odk.emit(schema, {
 *   targets: ["@typespec/openapi3", "@typespec/protobuf"],
 *   outDir: "./generated",
 * });
 * ```
 */
async function emit(schema: ODKSchema, options: EmitOptions): Promise<void> {
  await emitToTargets(schema.compiled, options);
}

/**
 * Generate TypeSpec source code from an ODK schema.
 *
 * @param schema - ODK schema created with odk.schema()
 * @param namespace - Optional namespace override
 * @returns TypeSpec source code as a string
 *
 * @example
 * ```typescript
 * const tspSource = odk.toTypeSpec(schema);
 * console.log(tspSource);
 * ```
 */
function toTypeSpec(schema: ODKSchema, namespace?: string): string {
  return compiledSchemaToTypeSpec(schema.compiled, namespace);
}

/**
 * Get the compiled schema representation (useful for the Rust CLI).
 *
 * @param schema - ODK schema created with odk.schema()
 * @returns CompiledSchema object
 */
function getCompiled(schema: ODKSchema): CompiledSchema {
  return schema.compiled;
}

/**
 * Serialize an ODK schema to JSON (for the Rust CLI).
 *
 * @param schema - ODK schema created with odk.schema()
 * @returns JSON string of the compiled schema
 *
 * @example
 * ```typescript
 * const json = odk.toJSON(schema);
 * await Bun.write("schema.json", json);
 * // Then: tspgen schema.json -o output.tsp
 * ```
 */
function toJSON(schema: ODKSchema): string {
  return JSON.stringify(schema.compiled, null, 2);
}

/**
 * ODK namespace - main entry point for the library.
 */
export const odk = {
  schema,
  emit,
  toTypeSpec,
  toJSON,
  getCompiled,
  raw,
  addImport,
} as const;
