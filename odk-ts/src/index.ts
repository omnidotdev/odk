/**
 * @omnidotdev/odk
 *
 * Define schemas with Zod, ArkType, or Valibot.
 * Emit to OpenAPI, Protobuf, JSON Schema, and more via TypeSpec.
 *
 * @example
 * ```typescript
 * import { odk } from "@omnidotdev/odk";
 * import { z } from "zod";
 *
 * const User = z.object({
 *   id: z.string().uuid(),
 *   email: z.string().email(),
 *   role: z.enum(["admin", "user", "guest"]),
 * });
 *
 * const schema = odk.schema("MyAPI", { User });
 *
 * // Emit to OpenAPI
 * await odk.emit(schema, {
 *   targets: ["@typespec/openapi3"],
 *   outDir: "./generated",
 * });
 *
 * // Or get TypeSpec source
 * const tsp = odk.toTypeSpec(schema);
 *
 * // Or JSON for the Rust CLI
 * const json = odk.toJSON(schema);
 * ```
 */

export { serializeSchema } from "./compile.js";
export { extractJsonSchema, getSchemaLibrary } from "./jsonSchemaExtract.js";
// Main API
export { odk } from "./odk.js";
export { standardSchemaToCompiled } from "./standardSchemaAdapter.js";
export { compiledSchemaToTypeSpec, emitToTargets } from "./typespecBridge.js";

// Core types
export type {
  CompiledField,
  CompiledModel,
  CompiledOperation,
  CompiledSchema,
  CompiledType,
  CompiledUnion,
  CompiledUnionVariant,
  Directive,
  FieldConstraints,
  FieldSelection,
  OperationKind,
  ScalarType,
  SelectionSet,
} from "./compile.js";
// JSON Schema utilities
export type { JSONSchema } from "./jsonSchemaExtract.js";
export type { ODKSchema, SchemaOptions } from "./odk.js";
// Standard Schema adapter
export type { AdapterOptions } from "./standardSchemaAdapter.js";
// TypeSpec bridge
export type { EmitOptions } from "./typespecBridge.js";
