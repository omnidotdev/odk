/**
 * TypeSpec Bridge
 *
 * Converts CompiledSchema to TypeSpec source code, then uses
 * the TypeSpec compiler to emit to various targets.
 */

import type {
  CompiledSchema,
  CompiledModel,
  CompiledUnion,
  CompiledField,
  CompiledType,
} from "./compile.js";

export interface EmitOptions {
  /** Target emitters (e.g., "@typespec/openapi3", "@typespec/protobuf") */
  targets: string[];
  /** Output directory */
  outDir: string;
  /** Namespace for generated TypeSpec */
  namespace?: string;
}

/**
 * Convert CompiledSchema to TypeSpec source code.
 */
export function compiledSchemaToTypeSpec(
  schema: CompiledSchema,
  namespace?: string,
): string {
  const lines: string[] = [];
  const ns = namespace || schema.name;

  // Imports
  lines.push('import "@typespec/json-schema";');
  lines.push("using TypeSpec.JsonSchema;");
  lines.push("");

  // Namespace
  lines.push(`namespace ${sanitizeIdentifier(ns)};`);
  lines.push("");

  // Emit unions first (enums)
  for (const union of schema.unions) {
    lines.push(emitUnion(union));
    lines.push("");
  }

  // Emit models
  for (const model of schema.models) {
    lines.push(emitModel(model));
    lines.push("");
  }

  return lines.join("\n");
}

function emitUnion(union: CompiledUnion): string {
  const lines: string[] = [];

  if (union.description) {
    lines.push(`/** ${union.description} */`);
  }

  // Check if it's a string literal enum
  const isStringEnum = union.variants.every(
    (v) => v.type?.kind === "literal" && typeof v.type.value === "string",
  );

  if (isStringEnum) {
    lines.push(`enum ${sanitizeIdentifier(union.name)} {`);
    for (const variant of union.variants) {
      const value = (variant.type as { kind: "literal"; value: string }).value;
      // If the name is a valid identifier, use it; otherwise quote it
      if (isValidIdentifier(variant.name)) {
        if (variant.name === value) {
          lines.push(`  ${variant.name},`);
        } else {
          lines.push(`  ${variant.name}: "${value}",`);
        }
      } else {
        lines.push(`  "${value}",`);
      }
    }
    lines.push("}");
  } else {
    // Union type
    const variantTypes = union.variants
      .map((v) => (v.type ? typeToString(v.type) : v.name))
      .join(" | ");
    lines.push(`alias ${sanitizeIdentifier(union.name)} = ${variantTypes};`);
  }

  return lines.join("\n");
}

function emitModel(model: CompiledModel): string {
  const lines: string[] = [];

  if (model.description) {
    lines.push(`/** ${model.description} */`);
  }

  lines.push("@jsonSchema");
  lines.push(`model ${sanitizeIdentifier(model.name)} {`);

  for (const field of model.fields) {
    lines.push(emitField(field));
  }

  lines.push("}");

  return lines.join("\n");
}

function emitField(field: CompiledField): string {
  const decorators: string[] = [];

  // Add constraint decorators
  if (field.constraints) {
    const c = field.constraints;
    if (c.minLength !== undefined)
      decorators.push(`@minLength(${c.minLength})`);
    if (c.maxLength !== undefined)
      decorators.push(`@maxLength(${c.maxLength})`);
    if (c.minValue !== undefined) decorators.push(`@minValue(${c.minValue})`);
    if (c.maxValue !== undefined) decorators.push(`@maxValue(${c.maxValue})`);
    if (c.pattern !== undefined)
      decorators.push(`@pattern("${escapeString(c.pattern)}")`);
    if (c.multipleOf !== undefined)
      decorators.push(`@multipleOf(${c.multipleOf})`);
    if (c.format !== undefined) {
      // Map common formats to TypeSpec decorators
      switch (c.format) {
        case "uuid":
          decorators.push('@format("uuid")');
          break;
        case "email":
          decorators.push('@format("email")');
          break;
        case "uri":
        case "url":
          decorators.push('@format("uri")');
          break;
        case "date":
          decorators.push('@format("date")');
          break;
        case "date-time":
          decorators.push('@format("date-time")');
          break;
        default:
          decorators.push(`@format("${c.format}")`);
      }
    }
  }

  const decoratorStr = decorators.length > 0 ? decorators.join(" ") + " " : "";
  const optionalMark = field.optional ? "?" : "";
  const typeStr = typeToString(field.type);
  const fieldName = sanitizeIdentifier(field.name);

  return `  ${decoratorStr}${fieldName}${optionalMark}: ${typeStr};`;
}

function typeToString(type: CompiledType): string {
  switch (type.kind) {
    case "scalar":
      return scalarToTypeSpec(type.name);
    case "array":
      return `${typeToString(type.items)}[]`;
    case "ref":
      return sanitizeIdentifier(type.name);
    case "literal":
      if (typeof type.value === "string") {
        return `"${escapeString(type.value)}"`;
      }
      return String(type.value);
    case "union":
      return type.variants.map(typeToString).join(" | ");
    case "object":
      const fields = type.fields
        .map((f) => {
          const opt = f.optional ? "?" : "";
          return `${f.name}${opt}: ${typeToString(f.type)}`;
        })
        .join("; ");
      return `{ ${fields} }`;
    default:
      return "unknown";
  }
}

function scalarToTypeSpec(scalar: string): string {
  switch (scalar) {
    case "string":
      return "string";
    case "int32":
      return "int32";
    case "int64":
      return "int64";
    case "float32":
      return "float32";
    case "float64":
      return "float64";
    case "boolean":
      return "boolean";
    case "date":
      return "plainDate";
    case "datetime":
      return "utcDateTime";
    case "bytes":
      return "bytes";
    default:
      return "unknown";
  }
}

function sanitizeIdentifier(name: string): string {
  // Replace invalid characters with underscores
  let sanitized = name.replace(/[^A-Za-z0-9_]/g, "_");
  // Ensure it doesn't start with a number
  if (/^[0-9]/.test(sanitized)) {
    sanitized = "_" + sanitized;
  }
  return sanitized || "_";
}

function isValidIdentifier(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}

function escapeString(s: string): string {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

/**
 * Emit CompiledSchema to target formats using TypeSpec compiler.
 *
 * This creates a temporary TypeSpec project with proper package.json
 * to resolve emitter dependencies.
 */
export async function emitToTargets(
  schema: CompiledSchema,
  options: EmitOptions,
): Promise<void> {
  const { targets, outDir, namespace } = options;

  const fs = await import("node:fs/promises");
  const path = await import("node:path");
  const os = await import("node:os");

  // Generate TypeSpec source
  const tspSource = compiledSchemaToTypeSpec(schema, namespace);

  // Create temp directory with a proper TypeSpec project structure
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "odk-"));
  const tspPath = path.join(tempDir, "main.tsp");

  try {
    // Write the TypeSpec source
    await fs.writeFile(tspPath, tspSource, "utf-8");

    // Create package.json with dependencies so TypeSpec can find them
    const packageJson = {
      name: "odk-temp-project",
      type: "module",
      dependencies: {
        "@typespec/compiler": "*",
        "@typespec/json-schema": "*",
        ...Object.fromEntries(targets.map((t) => [t, "*"])),
      },
    };
    await fs.writeFile(
      path.join(tempDir, "package.json"),
      JSON.stringify(packageJson, null, 2),
      "utf-8",
    );

    // Symlink node_modules from odk-ts to the temp directory
    // This avoids having to install packages
    const odkNodeModules = path.resolve(import.meta.dirname, "../node_modules");
    const tempNodeModules = path.join(tempDir, "node_modules");

    try {
      await fs.symlink(odkNodeModules, tempNodeModules, "dir");
    } catch {
      // If symlink fails (e.g., on Windows without admin), copy the approach below
      // For now, just try to compile with the current working directory
    }

    // Create tspconfig.yaml
    const absoluteOutDir = path.resolve(process.cwd(), outDir);
    const tspConfigContent = `
emit:
${targets.map((t) => `  - "${t}"`).join("\n")}
options:
${targets.map((t) => `  "${t}":\n    output-dir: "${absoluteOutDir}"`).join("\n")}
`;
    await fs.writeFile(
      path.join(tempDir, "tspconfig.yaml"),
      tspConfigContent,
      "utf-8",
    );

    // Ensure output directory exists
    await fs.mkdir(absoluteOutDir, { recursive: true });

    // Run TypeSpec compiler
    const { compile, NodeHost } = await import("@typespec/compiler");
    const program = await compile(NodeHost, tspPath, {
      outputDir: absoluteOutDir,
      emit: targets,
    });

    if (program.diagnostics.length > 0) {
      for (const diag of program.diagnostics) {
        if (diag.severity === "error") {
          console.error(`TypeSpec error: ${diag.message}`);
        } else {
          console.warn(`TypeSpec warning: ${diag.message}`);
        }
      }

      const errors = program.diagnostics.filter((d) => d.severity === "error");
      if (errors.length > 0) {
        throw new Error(
          `TypeSpec compilation failed with ${errors.length} errors`,
        );
      }
    }

    console.log(`Emitted to ${absoluteOutDir}`);
  } finally {
    // Cleanup temp files
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}
