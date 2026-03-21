#!/usr/bin/env node
// biome-ignore-all lint/suspicious/noConsole: CLI tool — console output is expected

/**
 * ODK CLI (TypeScript)
 *
 * Commands:
 *   odk compile <schema.ts>  Compile and emit schema
 *   odk init                 Initialize a new ODK project
 */

import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";

async function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  switch (command) {
    case "compile":
      await compileCommand(args.slice(1));
      break;
    case "init":
      await initCommand();
      break;
    case "--help":
    case "-h":
    case undefined:
      printHelp();
      break;
    default:
      console.error(`Unknown command: ${command}`);
      printHelp();
      process.exit(1);
  }
}

function printHelp() {
  console.log(`
ODK - Omni Development Kit

Usage:
  odk compile <schema.ts> [options]    Compile and emit schema
  odk init                             Initialize a new ODK project
  odk --help                           Show this help

Compile Options:
  --out, -o <dir>       Output directory (default: ./generated)
  --target, -t <name>   TypeSpec emitter (can be repeated)
                        Examples: @typespec/openapi3, @typespec/json-schema

Examples:
  odk compile schema.ts
  odk compile schema.ts -o ./out -t @typespec/openapi3 -t @typespec/json-schema
  odk init
`);
}

async function compileCommand(args: string[]) {
  if (args.length === 0) {
    console.error("Error: No schema file specified");
    console.error("Usage: odk compile <schema.ts>");
    process.exit(1);
  }

  const schemaPath = resolve(args[0]!);
  let outDir = "./generated";
  let targets: string[] = [];

  // Parse options
  for (let i = 1; i < args.length; i++) {
    if (args[i] === "--out" || args[i] === "-o") {
      outDir = args[++i] ?? outDir;
    } else if (args[i] === "--target" || args[i] === "-t") {
      const target = args[++i];
      if (target) targets.push(target);
    }
  }

  if (targets.length === 0) {
    targets = ["@typespec/json-schema", "@typespec/openapi3"];
  }

  console.log(`Compiling ${schemaPath}...`);
  console.log(`Targets: ${targets.join(", ")}`);
  console.log(`Output: ${outDir}`);

  try {
    // Dynamic import of the schema file
    const schemaModule = await import(schemaPath);

    // Look for exported schema (ODKSchema from odk.schema())
    const schema = schemaModule.default || schemaModule.schema;

    if (!schema) {
      console.error(
        "Error: No schema found. Export a schema as default or named 'schema'.",
      );
      process.exit(1);
    }

    // Import odk and emit
    const { odk } = await import("./odk.js");

    await odk.emit(schema, {
      targets,
      outDir: resolve(outDir),
    });

    console.log("Done!");
  } catch (error) {
    console.error("Compilation failed:", error);
    process.exit(1);
  }
}

async function initCommand() {
  console.log("Initializing ODK project...");

  // Create example schema using modern Zod-based API
  const exampleSchema = `import { odk } from "@omnidotdev/odk";
import { z } from "zod";

// Define your models with Zod
const UserRole = z.enum(["admin", "user", "guest"]);

const User = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  name: z.string().optional(),
  role: UserRole,
  createdAt: z.string().datetime(),
});

const Post = z.object({
  id: z.string().uuid(),
  title: z.string().min(1).max(200),
  content: z.string(),
  authorId: z.string().uuid(),
  status: z.enum(["draft", "published", "archived"]),
});

// Create and export the schema
export default odk.schema("MyApp", { User, Post, UserRole });
`;

  await writeFile("schema.ts", exampleSchema);
  console.log("Created schema.ts");

  console.log(`
Next steps:
  1. Install dependencies: bun add zod @omnidotdev/odk
  2. Edit schema.ts to define your models
  3. Run: odk compile schema.ts
  4. Find generated files in ./generated/
`);
}

main().catch(console.error);
