/**
 * Standard Schema Adapter
 *
 * Converts any Standard Schema-compliant library (Zod, ArkType, Valibot)
 * to ODK's CompiledSchema IR format.
 */

import { extractJsonSchema } from "./jsonSchemaExtract.js";

import type {
  CompiledField,
  CompiledModel,
  CompiledSchema,
  CompiledType,
  CompiledUnion,
  FieldConstraints,
  ScalarType,
} from "./compile.js";
import type { JSONSchema } from "./jsonSchemaExtract.js";

export interface AdapterOptions {
  /** Suppress warnings about non-serializable constraints */
  suppressWarnings?: boolean;
}

/**
 * Convert a collection of Standard Schema types to CompiledSchema.
 */
export function standardSchemaToCompiled(
  name: string,
  types: Record<string, unknown>,
  options: AdapterOptions = {},
): CompiledSchema {
  const models: CompiledModel[] = [];
  const unions: CompiledUnion[] = [];
  const warnings: string[] = [];
  const processedDefs = new Set<string>();

  for (const [typeName, schema] of Object.entries(types)) {
    // Check for non-serializable constraints
    checkSerializability(schema, typeName, warnings);

    // Extract JSON Schema representation
    const jsonSchema = extractJsonSchema(schema);

    // Process any $defs/definitions first
    const defs = { ...jsonSchema.$defs, ...jsonSchema.definitions };
    for (const [defName, defSchema] of Object.entries(defs)) {
      if (processedDefs.has(defName)) continue;
      processedDefs.add(defName);

      const result = jsonSchemaToType(defName, defSchema);
      if (result.model) models.push(result.model);
      if (result.union) unions.push(result.union);
    }

    // Process the main type
    const result = jsonSchemaToType(typeName, jsonSchema);
    if (result.model) models.push(result.model);
    if (result.union) unions.push(result.union);
  }

  // Emit warnings
  if (!options.suppressWarnings) {
    for (const warning of warnings) {
      console.warn(`\u26A0\uFE0F  ${warning}`);
    }
  }

  return { name, models, unions, warnings };
}

interface TypeConversionResult {
  model?: CompiledModel;
  union?: CompiledUnion;
  type?: CompiledType;
}

function jsonSchemaToType(
  name: string,
  schema: JSONSchema,
): TypeConversionResult {
  // Handle enums as unions
  if (schema.enum) {
    return {
      union: {
        name,
        variants: schema.enum.map((v) => ({
          name: String(v),
          type: { kind: "literal", value: v },
        })),
        description: schema.description,
      },
    };
  }

  // Handle oneOf/anyOf as unions
  const variants = schema.oneOf ?? schema.anyOf;
  if (variants) {
    return {
      union: {
        name,
        variants: variants.map((v, i) => {
          const variantName = v.title || `Variant${i + 1}`;
          return {
            name: variantName,
            type: jsonSchemaPropertyToType(v),
          };
        }),
        description: schema.description,
      },
    };
  }

  // Handle objects as models
  if (schema.type === "object" || schema.properties) {
    const fields: CompiledField[] = [];
    const required = new Set(schema.required || []);

    for (const [propName, propSchema] of Object.entries(
      schema.properties || {},
    )) {
      fields.push({
        name: propName,
        type: jsonSchemaPropertyToType(propSchema),
        optional: !required.has(propName),
        description: propSchema.description,
        constraints: extractConstraints(propSchema),
      });
    }

    return {
      model: {
        name,
        fields,
        description: schema.description,
      },
    };
  }

  // Handle arrays
  if (schema.type === "array") {
    return {
      type: {
        kind: "array",
        items: schema.items
          ? jsonSchemaPropertyToType(schema.items)
          : { kind: "scalar", name: "unknown" },
      },
    };
  }

  // Handle refs
  if (schema.$ref) {
    const refName = schema.$ref.split("/").pop() || "Unknown";
    return {
      type: { kind: "ref", name: refName },
    };
  }

  // Handle primitives - create an alias
  return {
    type: jsonSchemaPropertyToType(schema),
  };
}

function jsonSchemaPropertyToType(schema: JSONSchema): CompiledType {
  // Handle refs
  if (schema.$ref) {
    const refName = schema.$ref.split("/").pop() || "Unknown";
    return { kind: "ref", name: refName };
  }

  // Handle const
  if (schema.const !== undefined) {
    return {
      kind: "literal",
      value: schema.const as string | number | boolean,
    };
  }

  // Handle enum (inline)
  if (schema.enum) {
    return {
      kind: "union",
      variants: schema.enum.map((v) => ({
        kind: "literal" as const,
        value: v,
      })),
    };
  }

  // Handle oneOf/anyOf
  const unionVariants = schema.oneOf ?? schema.anyOf;
  if (unionVariants) {
    return {
      kind: "union",
      variants: unionVariants.map((v) => jsonSchemaPropertyToType(v)),
    };
  }

  // Handle arrays
  if (schema.type === "array") {
    return {
      kind: "array",
      items: schema.items
        ? jsonSchemaPropertyToType(schema.items)
        : { kind: "scalar", name: "unknown" },
    };
  }

  // Handle objects (inline)
  if (schema.type === "object" || schema.properties) {
    const fields: CompiledField[] = [];
    const required = new Set(schema.required || []);

    for (const [propName, propSchema] of Object.entries(
      schema.properties || {},
    )) {
      fields.push({
        name: propName,
        type: jsonSchemaPropertyToType(propSchema),
        optional: !required.has(propName),
        constraints: extractConstraints(propSchema),
      });
    }

    return { kind: "object", fields };
  }

  // Handle type arrays (union of types)
  if (Array.isArray(schema.type)) {
    return {
      kind: "union",
      variants: schema.type.map((t) => ({
        kind: "scalar" as const,
        name: jsonTypeToScalar(t),
      })),
    };
  }

  // Handle primitive types
  if (typeof schema.type === "string") {
    return { kind: "scalar", name: jsonTypeToScalar(schema.type) };
  }

  return { kind: "scalar", name: "unknown" };
}

function jsonTypeToScalar(type: string): ScalarType {
  switch (type) {
    case "string":
      return "string";
    case "number":
      return "float64";
    case "integer":
      return "int64";
    case "boolean":
      return "boolean";
    case "null":
      return "unknown"; // TypeSpec doesn't have null, handle via optional
    default:
      return "unknown";
  }
}

function extractConstraints(schema: JSONSchema): FieldConstraints | undefined {
  const constraints: FieldConstraints = {};
  let hasConstraints = false;

  if (schema.minLength !== undefined) {
    constraints.minLength = schema.minLength;
    hasConstraints = true;
  }
  if (schema.maxLength !== undefined) {
    constraints.maxLength = schema.maxLength;
    hasConstraints = true;
  }
  if (schema.minimum !== undefined) {
    constraints.minValue = schema.minimum;
    hasConstraints = true;
  }
  if (schema.maximum !== undefined) {
    constraints.maxValue = schema.maximum;
    hasConstraints = true;
  }
  if (schema.exclusiveMinimum !== undefined) {
    constraints.minValue = schema.exclusiveMinimum;
    hasConstraints = true;
  }
  if (schema.exclusiveMaximum !== undefined) {
    constraints.maxValue = schema.exclusiveMaximum;
    hasConstraints = true;
  }
  if (schema.pattern !== undefined) {
    constraints.pattern = schema.pattern;
    hasConstraints = true;
  }
  if (schema.format !== undefined) {
    constraints.format = schema.format;
    hasConstraints = true;
  }
  if (schema.multipleOf !== undefined) {
    constraints.multipleOf = schema.multipleOf;
    hasConstraints = true;
  }

  return hasConstraints ? constraints : undefined;
}

/**
 * Check for constraints that cannot be serialized to TypeSpec.
 */
function checkSerializability(
  schema: unknown,
  typeName: string,
  warnings: string[],
): void {
  // Zod-specific checks
  if (isZodSchemaWithDef(schema)) {
    const def = schema._def;

    // Check for transforms
    if (def.typeName === "ZodEffects" && def.effect?.type === "transform") {
      warnings.push(
        `${typeName}: .transform() is not serializable - will be ignored in emission`,
      );
    }

    // Check for refines
    if (def.typeName === "ZodEffects" && def.effect?.type === "refinement") {
      warnings.push(
        `${typeName}: .refine()/.superRefine() is not serializable - will be ignored in emission`,
      );
    }

    // Check for preprocessors
    if (def.typeName === "ZodEffects" && def.effect?.type === "preprocess") {
      warnings.push(
        `${typeName}: .preprocess() is not serializable - will be ignored in emission`,
      );
    }
  }
}

function isZodSchemaWithDef(
  schema: unknown,
): schema is { _def: { typeName?: string; effect?: { type: string } } } {
  return (
    schema !== null &&
    typeof schema === "object" &&
    "_def" in schema &&
    typeof (schema as Record<string, unknown>)._def === "object"
  );
}
