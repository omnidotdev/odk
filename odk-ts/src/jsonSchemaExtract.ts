/**
 * JSON Schema extraction from different Standard Schema libraries.
 * Each library has its own method for converting to JSON Schema.
 */

import type { StandardSchemaV1 } from "@standard-schema/spec";

export interface JSONSchema {
  type?: string | string[];
  properties?: Record<string, JSONSchema>;
  required?: string[];
  items?: JSONSchema;
  enum?: (string | number | boolean)[];
  oneOf?: JSONSchema[];
  anyOf?: JSONSchema[];
  allOf?: JSONSchema[];
  $ref?: string;
  $defs?: Record<string, JSONSchema>;
  definitions?: Record<string, JSONSchema>;
  description?: string;
  title?: string;
  // Constraints
  minLength?: number;
  maxLength?: number;
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  pattern?: string;
  format?: string;
  multipleOf?: number;
  // Additional
  additionalProperties?: boolean | JSONSchema;
  default?: unknown;
  const?: unknown;
}

/**
 * Detect which schema library the schema comes from and extract JSON Schema.
 */
export function extractJsonSchema(schema: unknown): JSONSchema {
  // Zod detection
  if (isZodSchema(schema)) {
    return extractFromZod(schema);
  }

  // ArkType detection
  if (isArkTypeSchema(schema)) {
    return extractFromArkType(schema);
  }

  // Valibot detection
  if (isValibotSchema(schema)) {
    return extractFromValibot(schema);
  }

  // Check if it's already a Standard Schema with ~standard property
  if (isStandardSchema(schema)) {
    // Try to extract via library-specific methods first
    // Fall back to inferring from the standard schema interface
    return inferFromStandardSchema(schema);
  }

  throw new Error(
    "Unknown schema library. Please use Zod, ArkType, or Valibot.",
  );
}

// ============ Zod ============

// Zod 4 toJSONSchema options
interface Zod4JsonSchemaOptions {
  unrepresentable?: "any" | "throw";
}

// Zod 4 schema detection (has toJSONSchema method and ~standard.vendor === "zod")
function isZod4Schema(schema: unknown): schema is {
  toJSONSchema: (opts?: Zod4JsonSchemaOptions) => JSONSchema;
  "~standard": { vendor: string };
} {
  return (
    schema !== null &&
    typeof schema === "object" &&
    "toJSONSchema" in schema &&
    typeof (schema as Record<string, unknown>).toJSONSchema === "function" &&
    "~standard" in schema &&
    (schema as { "~standard": { vendor: string } })["~standard"]?.vendor ===
      "zod"
  );
}

// Zod 3 schema detection (has _def property)
function isZod3Schema(schema: unknown): schema is { _def: unknown } {
  return (
    schema !== null &&
    typeof schema === "object" &&
    "_def" in schema &&
    typeof (schema as Record<string, unknown>)._def === "object" &&
    !("toJSONSchema" in schema) // Not Zod 4
  );
}

function isZodSchema(schema: unknown): boolean {
  return isZod4Schema(schema) || isZod3Schema(schema);
}

function extractFromZod(schema: unknown): JSONSchema {
  // Zod 4: Use native toJSONSchema()
  if (isZod4Schema(schema)) {
    // Use unrepresentable: 'any' to avoid throwing on Date, Function, etc.
    // These will become empty schemas {} which we handle gracefully
    return schema.toJSONSchema({ unrepresentable: "any" }) as JSONSchema;
  }

  // Zod 3: Use zod-to-json-schema library
  if (isZod3Schema(schema)) {
    try {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const { zodToJsonSchema } = require("zod-to-json-schema");
      return zodToJsonSchema(schema, { $refStrategy: "none" }) as JSONSchema;
    } catch {
      throw new Error(
        "zod-to-json-schema is required for Zod 3 schemas. Install it with: bun add zod-to-json-schema",
      );
    }
  }

  throw new Error("Unknown Zod schema version");
}

// ============ ArkType ============

function isArkTypeSchema(schema: unknown): boolean {
  // ArkType schemas are functions (callable validators), not plain objects
  if (
    schema === null ||
    (typeof schema !== "object" && typeof schema !== "function")
  )
    return false;

  // Check for ~standard.vendor === "arktype"
  if (
    "~standard" in schema &&
    (schema as { "~standard": { vendor: string } })["~standard"]?.vendor ===
      "arktype"
  ) {
    return true;
  }

  // Fallback: check for toJsonSchema method (lowercase, on prototype)
  return (
    typeof (schema as { toJsonSchema?: unknown }).toJsonSchema === "function"
  );
}

function extractFromArkType(schema: unknown): JSONSchema {
  const s = schema as { toJsonSchema: () => JSONSchema };
  return s.toJsonSchema();
}

// ============ Valibot ============

function isValibotSchema(
  schema: unknown,
): schema is { kind: string; type: string } {
  return (
    schema !== null &&
    typeof schema === "object" &&
    "kind" in schema &&
    (schema as Record<string, unknown>).kind === "schema"
  );
}

function extractFromValibot(schema: unknown): JSONSchema {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const { toJsonSchema } = require("@valibot/to-json-schema");
    return toJsonSchema(schema) as JSONSchema;
  } catch {
    throw new Error(
      "@valibot/to-json-schema is required for Valibot schemas. Install it with: bun add @valibot/to-json-schema",
    );
  }
}

// ============ Standard Schema Fallback ============

function isStandardSchema(schema: unknown): schema is StandardSchemaV1 {
  return (
    schema !== null &&
    typeof schema === "object" &&
    "~standard" in schema &&
    typeof (schema as Record<string, unknown>)["~standard"] === "object"
  );
}

function inferFromStandardSchema(schema: StandardSchemaV1): JSONSchema {
  // Standard Schema spec doesn't mandate JSON Schema output,
  // so we can only infer basic structure from the types property
  const standard = schema["~standard"];

  // If the schema library provides vendor-specific JSON Schema, use it
  if ("toJsonSchema" in schema && typeof schema.toJsonSchema === "function") {
    return (schema as { toJsonSchema: () => JSONSchema }).toJsonSchema();
  }

  // Otherwise, return a minimal schema with unknown type
  // The adapter will warn about this
  return {
    type: "object",
    description: `Standard Schema (vendor: ${standard.vendor})`,
  };
}

/**
 * Get the library name from a schema for error messages.
 */
export function getSchemaLibrary(schema: unknown): string {
  if (isZod4Schema(schema) || isZod3Schema(schema)) return "zod";
  if (isArkTypeSchema(schema)) return "arktype";
  if (isValibotSchema(schema)) return "valibot";
  if (isStandardSchema(schema)) {
    return (schema as StandardSchemaV1)["~standard"].vendor;
  }
  return "unknown";
}
