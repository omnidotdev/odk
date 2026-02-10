/**
 * Test: odk.emit() end-to-end with TypeSpec compiler
 *
 * This example tests emitting to OpenAPI via TypeSpec.
 */

import { odk } from "../src/index.js";
import { z } from "zod";
import { mkdir } from "node:fs/promises";

// Simple schema for testing
const UserRole = z.enum(["admin", "user", "guest"]);

const User = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  name: z.string().min(1).max(100),
  role: UserRole,
  createdAt: z.iso.datetime(),
});

const schema = odk.schema("TestAPI", { UserRole, User });

// First, let's output the TypeSpec source to verify it's valid
console.log("=== Generated TypeSpec ===\n");
console.log(odk.toTypeSpec(schema));

// Create output directory
await mkdir("./examples/generated", { recursive: true });

// Test emit to JSON Schema (better for models-only)
console.log("\n=== Emitting to JSON Schema... ===\n");

try {
  await odk.emit(schema, {
    targets: ["@typespec/json-schema"],
    outDir: "./examples/generated",
  });
  console.log("Success! Check ./examples/generated/");
} catch (error) {
  console.error("Emit failed:", error);
}
