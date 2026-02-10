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

// Main API
export { odk } from "./odk.js";
export type { ODKSchema, SchemaOptions } from "./odk.js";

// Core types
export type {
  CompiledSchema,
  CompiledModel,
  CompiledUnion,
  CompiledField,
  CompiledType,
  CompiledUnionVariant,
  FieldConstraints,
  ScalarType,
} from "./compile.js";
export { serializeSchema } from "./compile.js";

// TypeSpec bridge
export type { EmitOptions } from "./typespecBridge.js";
export { compiledSchemaToTypeSpec, emitToTargets } from "./typespecBridge.js";

// Standard Schema adapter
export type { AdapterOptions } from "./standardSchemaAdapter.js";
export { standardSchemaToCompiled } from "./standardSchemaAdapter.js";

// JSON Schema utilities
export type { JSONSchema } from "./jsonSchemaExtract.js";
export { extractJsonSchema, getSchemaLibrary } from "./jsonSchemaExtract.js";
