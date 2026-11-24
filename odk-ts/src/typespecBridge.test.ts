import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { z } from "zod";

import { odk } from "./index.js";
import { findTypeSpecNodeModules } from "./typespecBridge.js";

const tempDirs: string[] = [];

const makeTempDir = () => {
  const dir = mkdtempSync(path.join(tmpdir(), "odk-test-"));
  tempDirs.push(dir);
  return dir;
};

afterAll(() => {
  for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true });
});

/** Render a single Zod object as TypeSpec */
const tspFor = (shape: z.ZodRawShape) =>
  odk.toTypeSpec(
    odk.schema("Test", { Input: z.object(shape) }, { suppressWarnings: true }),
  );

describe("Zod to TypeSpec fidelity", () => {
  test("integers emit as safeint without the synthetic safe-integer bounds", () => {
    const tsp = tspFor({ seed: z.number().int(), count: z.int() });

    expect(tsp).toContain("  seed: safeint;");
    expect(tsp).toContain("  count: safeint;");
    expect(tsp).not.toContain("int64");
    expect(tsp).not.toContain("9007199254740991");
  });

  test("integers with explicit bounds that fit 32 bits emit as int32", () => {
    const tsp = tspFor({ steps: z.number().int().min(1).max(150) });

    expect(tsp).toContain("@minValue(1) @maxValue(150) steps: int32;");
  });

  test("integers with one explicit bound keep it and stay safeint", () => {
    const tsp = tspFor({ n: z.number().int().min(1) });

    expect(tsp).toContain("@minValue(1) n: safeint;");
    expect(tsp).not.toContain("@maxValue");
  });

  test("describe() emits @doc on properties and models", () => {
    const tsp = odk.toTypeSpec(
      odk.schema("Test", {
        Input: z
          .object({ prompt: z.string().describe('What "to" generate') })
          .describe("Task input"),
      }),
    );

    expect(tsp).toContain('@doc("Task input")\n@jsonSchema\nmodel Input {');
    expect(tsp).toContain('@doc("What \\"to\\" generate") prompt: string;');
  });

  test("default() makes the field optional and emits the default", () => {
    const tsp = tspFor({
      mode: z.enum(["fast", "best"]).default("fast"),
      steps: z.number().int().min(1).max(50).default(20),
      tags: z.array(z.string()).default(["a"]),
      hd: z.boolean().default(false),
    });

    expect(tsp).toContain('  mode?: "fast" | "best" = "fast";');
    expect(tsp).toContain("@minValue(1) @maxValue(50) steps?: int32 = 20;");
    expect(tsp).toContain('  tags?: string[] = #["a"];');
    expect(tsp).toContain("  hd?: boolean = false;");
  });

  test("nullable() emits T | null instead of an unknown variant", () => {
    const tsp = tspFor({
      mask: z.string().nullable(),
      maybe: z.string().nullable().optional(),
      level: z.enum(["low", "high"]).nullable(),
      seed: z.number().int().nullable().default(null),
    });

    expect(tsp).toContain("  mask: string | null;");
    expect(tsp).toContain("  maybe?: string | null;");
    expect(tsp).toContain('  level: "low" | "high" | null;');
    expect(tsp).toContain("  seed?: safeint | null = null;");
    expect(tsp).not.toContain("unknown");
  });

  test("enums emit as string-literal unions inline and enums at top level", () => {
    const Mode = z.enum(["fast", "best"]);
    const tsp = odk.toTypeSpec(
      odk.schema("Test", { Mode, Input: z.object({ mode: Mode }) }),
    );

    expect(tsp).toContain("@jsonSchema\nenum Mode {\n  fast,\n  best,\n}");
    expect(tsp).toContain('  mode: "fast" | "best";');
  });

  test("arrays keep item types and length bounds", () => {
    const tsp = tspFor({
      images: z.array(z.string()).min(1).max(4),
      sizes: z.array(z.number().int()),
    });

    expect(tsp).toContain("@minItems(1) @maxItems(4) images: string[];");
    expect(tsp).toContain("  sizes: safeint[];");
  });

  test("raw blocks and imports are emitted", () => {
    const schema = odk.schema("Test", { Input: z.object({ a: z.string() }) });
    odk.addImport(schema, "@typespec/http");
    odk.raw(schema, '@route("/x") op x(): Input;', "Routes");

    const tsp = odk.toTypeSpec(schema);

    expect(tsp).toContain('import "@typespec/http";');
    expect(tsp).toContain("using TypeSpec.Http;");
    expect(tsp).toContain('// Routes\n@route("/x") op x(): Input;');
  });
});

describe("findTypeSpecNodeModules", () => {
  const install = (root: string, pkg: string) =>
    mkdirSync(path.join(root, "node_modules", pkg), { recursive: true });

  test("finds the consumer's node_modules from a nested directory", () => {
    const root = makeTempDir();
    install(root, "@typespec/compiler");
    install(root, "@typespec/json-schema");
    const nested = path.join(root, "src", "deep");
    mkdirSync(nested, { recursive: true });

    expect(findTypeSpecNodeModules([nested], ["@typespec/json-schema"])).toBe(
      path.join(root, "node_modules"),
    );
  });

  test("prefers a node_modules that also has every requested emitter", () => {
    const outer = makeTempDir();
    install(outer, "@typespec/compiler");
    install(outer, "@typespec/openapi3");
    const inner = path.join(outer, "pkg");
    install(inner, "@typespec/compiler");

    expect(findTypeSpecNodeModules([inner], ["@typespec/openapi3"])).toBe(
      path.join(outer, "node_modules"),
    );
  });

  test("falls back to the first compiler and then to undefined", () => {
    const root = makeTempDir();
    install(root, "@typespec/compiler");

    expect(findTypeSpecNodeModules([root], ["@typespec/missing"])).toBe(
      path.join(root, "node_modules"),
    );
    expect(findTypeSpecNodeModules([makeTempDir()], [])).toBeUndefined();
  });
});

describe("emit", () => {
  test("emits JSON Schema and OpenAPI with integers, docs, defaults, and null", async () => {
    const outDir = makeTempDir();
    const schema = odk.schema("Tasks", {
      Input: z.object({
        seed: z.number().int().optional(),
        mode: z.enum(["fast", "best"]).default("fast"),
        mask: z.string().nullable().optional(),
        prompt: z.string().describe("What to generate"),
      }),
    });

    await odk.emit(schema, {
      targets: ["@typespec/json-schema", "@typespec/openapi3"],
      outDir,
    });

    const jsonSchema = readFileSync(
      path.join(outDir, "@typespec/json-schema/Input.yaml"),
      "utf-8",
    );
    expect(jsonSchema).toContain("seed:\n    type: integer\n");
    expect(jsonSchema).toContain("default: fast");
    expect(jsonSchema).toContain('- type: "null"');
    expect(jsonSchema).toContain("description: What to generate");
    expect(jsonSchema).toContain("required:\n  - prompt\n");

    const openapi = readFileSync(
      path.join(outDir, "@typespec/openapi3/openapi.yaml"),
      "utf-8",
    );
    expect(openapi).toContain("Input:");
    expect(openapi).toContain("nullable: true");
    expect(openapi).toContain("default: fast");
    expect(openapi).toContain("description: What to generate");
  });
});
