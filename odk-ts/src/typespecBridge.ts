/**
 * TypeSpec Bridge
 *
 * Converts CompiledSchema to TypeSpec source code, then uses
 * the TypeSpec compiler to emit to various targets.
 */

import { existsSync } from "node:fs";
import path from "node:path";

import type {
  CompiledField,
  CompiledModel,
  CompiledSchema,
  CompiledType,
  CompiledUnion,
  ScalarType,
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

  // Imports: JSON Schema is always needed for the @jsonSchema decorator, the
  // rest come from the schema (odk.addImport / SchemaOptions.imports)
  const imports = [
    "@typespec/json-schema",
    ...(schema.imports ?? []).filter((i) => i !== "@typespec/json-schema"),
  ];
  const uniqueImports = [...new Set(imports)];
  for (const importPath of uniqueImports) {
    lines.push(`import "${escapeString(importPath)}";`);
  }
  lines.push("using TypeSpec.JsonSchema;");
  for (const importPath of uniqueImports) {
    const usingNs = importToUsingNamespace(importPath);
    if (usingNs) lines.push(`using ${usingNs};`);
  }
  lines.push("");

  // Namespace, declared as a service so the OpenAPI emitter includes its
  // models as component schemas even when no operation references them
  lines.push(`@service(#{ title: "${escapeString(ns)}" })`);
  lines.push(`namespace ${sanitizeIdentifier(ns)};`);
  lines.push("");

  // Emit unions first (enums)
  for (const union of schema.unions) {
    lines.push(emitUnion(union, schema));
    lines.push("");
  }

  // Emit models
  for (const model of schema.models) {
    lines.push(emitModel(model, schema));
    lines.push("");
  }

  // Emit raw TypeSpec blocks (odk.raw escape hatch)
  for (const block of schema.rawBlocks ?? []) {
    if (block.description) lines.push(`// ${block.description}`);
    lines.push(block.content);
    lines.push("");
  }

  return lines.join("\n");
}

/**
 * Map a TypeSpec library import to the namespace its decorators live in, so
 * they resolve without qualification (mirrors the Rust CLI bridge).
 */
function importToUsingNamespace(importPath: string): string | undefined {
  switch (importPath) {
    case "@typespec/http":
      return "TypeSpec.Http";
    case "@typespec/rest":
      return "TypeSpec.Rest";
    case "@typespec/openapi":
    case "@typespec/openapi3":
      return "TypeSpec.OpenAPI";
    case "@typespec/protobuf":
      return "TypeSpec.Protobuf";
    case "@typespec/versioning":
      return "TypeSpec.Versioning";
    case "@typespec/xml":
      return "TypeSpec.Xml";
    case "@typespec/events":
      return "TypeSpec.Events";
    case "@typespec/streams":
      return "TypeSpec.Streams";
    case "@typespec/sse":
      return "TypeSpec.SSE";
    case "@omnidotdev/typespec-drizzle":
      return "Drizzle";
    default:
      return undefined;
  }
}

function emitUnion(union: CompiledUnion, schema: CompiledSchema): string {
  const lines: string[] = [];

  if (union.description) {
    lines.push(docDecorator(union.description));
  }

  // Check if it's a string literal enum
  const isStringEnum = union.variants.every(
    (v) => v.type?.kind === "literal" && typeof v.type.value === "string",
  );

  if (isStringEnum) {
    lines.push("@jsonSchema");
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
      .map((v) => (v.type ? typeToString(v.type, schema) : v.name))
      .join(" | ");
    lines.push(`alias ${sanitizeIdentifier(union.name)} = ${variantTypes};`);
  }

  return lines.join("\n");
}

function emitModel(model: CompiledModel, schema: CompiledSchema): string {
  const lines: string[] = [];

  if (model.description) {
    lines.push(docDecorator(model.description));
  }

  lines.push("@jsonSchema");
  lines.push(`model ${sanitizeIdentifier(model.name)} {`);

  for (const field of model.fields) {
    lines.push(`  ${emitField(field, schema)};`);
  }

  lines.push("}");

  return lines.join("\n");
}

function emitField(field: CompiledField, schema: CompiledSchema): string {
  const decorators: string[] = [];

  if (field.description) {
    decorators.push(docDecorator(field.description));
  }

  // Add constraint decorators
  if (field.constraints) {
    const c = field.constraints;
    if (c.minLength !== undefined)
      decorators.push(`@minLength(${c.minLength})`);
    if (c.maxLength !== undefined)
      decorators.push(`@maxLength(${c.maxLength})`);
    if (c.minValue !== undefined) decorators.push(`@minValue(${c.minValue})`);
    if (c.maxValue !== undefined) decorators.push(`@maxValue(${c.maxValue})`);
    if (c.minItems !== undefined) decorators.push(`@minItems(${c.minItems})`);
    if (c.maxItems !== undefined) decorators.push(`@maxItems(${c.maxItems})`);
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
          decorators.push(`@format("${escapeString(c.format)}")`);
      }
    }
  }

  const decoratorStr = decorators.length > 0 ? `${decorators.join(" ")} ` : "";
  // A defaulted field is optional: TypeSpec only allows defaults on optional
  // properties, and callers may omit it
  const defaultValue = defaultToTypeSpec(field.default, field.type, schema);
  const optionalMark = field.optional || defaultValue !== undefined ? "?" : "";
  const baseType = typeToString(field.type, schema);
  const typeStr = field.nullable ? `${baseType} | null` : baseType;
  const defaultStr = defaultValue !== undefined ? ` = ${defaultValue}` : "";
  const fieldName = sanitizeIdentifier(field.name);

  return `${decoratorStr}${fieldName}${optionalMark}: ${typeStr}${defaultStr}`;
}

/** Scalars whose defaults are plain value literals */
const LITERAL_DEFAULT_SCALARS = new Set<ScalarType>([
  "string",
  "int32",
  "int64",
  "safeint",
  "float32",
  "float64",
  "boolean",
  "unknown",
]);

/**
 * Render a field default as a TypeSpec value expression.
 *
 * Returns `undefined` when the default cannot be expressed for the field's
 * type (e.g. a date-time scalar, which needs a constructor call), so the field
 * is emitted without a default instead of failing compilation.
 */
function defaultToTypeSpec(
  value: unknown,
  type: CompiledType,
  schema: CompiledSchema,
): string | undefined {
  if (value === undefined) return undefined;

  // A reference to a string enum takes a member reference, not a literal
  if (type.kind === "ref") {
    const union = schema.unions.find((u) => u.name === type.name);
    const variant = union?.variants.find(
      (v) => v.type?.kind === "literal" && v.type.value === value,
    );
    if (!union || !variant || !isValidIdentifier(variant.name)) {
      return undefined;
    }
    return `${sanitizeIdentifier(union.name)}.${variant.name}`;
  }

  if (type.kind === "scalar" && !LITERAL_DEFAULT_SCALARS.has(type.name)) {
    return undefined;
  }

  return valueToTypeSpec(value);
}

/** Render a JSON value as a TypeSpec value literal */
function valueToTypeSpec(value: unknown): string | undefined {
  if (value === null) return "null";
  if (typeof value === "string") return `"${escapeString(value)}"`;
  if (typeof value === "number") {
    return Number.isFinite(value) ? String(value) : undefined;
  }
  if (typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    const items = value.map(valueToTypeSpec);
    if (items.some((i) => i === undefined)) return undefined;
    return `#[${items.join(", ")}]`;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).map(
      ([k, v]) => {
        const rendered = valueToTypeSpec(v);
        if (rendered === undefined) return undefined;
        const key = isValidIdentifier(k) ? k : `\`${k.replace(/`/g, "\\`")}\``;
        return `${key}: ${rendered}`;
      },
    );
    if (entries.some((e) => e === undefined)) return undefined;
    return `#{${entries.join(", ")}}`;
  }
  return undefined;
}

/** Render a description as a @doc decorator */
function docDecorator(description: string): string {
  return `@doc("${escapeString(description)}")`;
}

function typeToString(type: CompiledType, schema: CompiledSchema): string {
  switch (type.kind) {
    case "scalar":
      return scalarToTypeSpec(type.name);
    case "array": {
      const items = typeToString(type.items, schema);
      // Parenthesize unions so `[]` applies to the whole union
      return type.items.kind === "union" ? `(${items})[]` : `${items}[]`;
    }
    case "ref":
      return sanitizeIdentifier(type.name);
    case "literal":
      if (typeof type.value === "string") {
        return `"${escapeString(type.value)}"`;
      }
      return String(type.value);
    case "union":
      return type.variants.map((v) => typeToString(v, schema)).join(" | ");
    case "object": {
      const fields = type.fields.map((f) => emitField(f, schema)).join("; ");
      return `{ ${fields} }`;
    }
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
    case "safeint":
      return "safeint";
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
    sanitized = `_${sanitized}`;
  }
  return sanitized || "_";
}

function isValidIdentifier(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}

function escapeString(s: string): string {
  return s
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\n/g, "\\n")
    .replace(/\r/g, "\\r")
    .replace(/\t/g, "\\t")
    .replace(/\$\{/g, "\\${");
}

/**
 * Find the `node_modules` directory that provides the TypeSpec toolchain.
 *
 * Walks up from each start directory (in order) the way Node resolves
 * packages, and returns the first `node_modules` containing
 * `@typespec/compiler` and every requested package. If none has them all, the
 * first one containing the compiler wins, so emitter resolution errors still
 * come from the compiler with a useful message. Returns `undefined` when no
 * compiler is found.
 */
export function findTypeSpecNodeModules(
  startDirs: string[],
  packages: string[],
): string | undefined {
  let compilerOnly: string | undefined;

  for (const start of startDirs) {
    let dir = path.resolve(start);
    while (true) {
      const candidate = path.join(dir, "node_modules");
      if (existsSync(path.join(candidate, "@typespec", "compiler"))) {
        if (packages.every((p) => existsSync(path.join(candidate, p)))) {
          return candidate;
        }
        compilerOnly ??= candidate;
      }

      const parent = path.dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
  }

  return compilerOnly;
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

    // Symlink the node_modules holding the TypeSpec toolchain into the temp
    // project so the compiler can resolve the imported emitters without an
    // install. This is the consumer's node_modules when ODK is a dependency
    const toolchainNodeModules =
      findTypeSpecNodeModules(
        [process.cwd(), import.meta.dirname],
        ["@typespec/json-schema", ...targets],
      ) ?? path.resolve(import.meta.dirname, "../node_modules");
    const tempNodeModules = path.join(tempDir, "node_modules");

    try {
      await fs.symlink(toolchainNodeModules, tempNodeModules, "dir");
    } catch {
      // Symlinks can fail (e.g. Windows without privileges); compilation then
      // falls back to resolving from the temp directory's ancestors
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

    // biome-ignore lint/suspicious/noConsole: intentional emit-completion notice for CLI output
    console.log(`Emitted to ${absoluteOutDir}`);
  } finally {
    // Cleanup temp files
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}
