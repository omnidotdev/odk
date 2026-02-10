/**
 * Example: Blog API with Zod
 *
 * This example demonstrates using ODK with Zod schemas to:
 * 1. Define runtime-validated types
 * 2. Generate TypeSpec source
 * 3. Emit to OpenAPI (and other targets)
 */

import { odk } from "../src/index.js";
import { z } from "zod";

// ============ Schema Definitions ============

const UserRole = z.enum(["admin", "author", "reader"]);

const User = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  name: z.string().min(1).max(100),
  role: UserRole,
  createdAt: z.iso.datetime(), // ISO 8601 datetime string
  bio: z.string().max(500).optional(),
});

const Post = z.object({
  id: z.string().uuid(),
  title: z.string().min(1).max(200),
  slug: z.string().regex(/^[a-z0-9-]+$/),
  content: z.string(),
  authorId: z.string().uuid(),
  published: z.boolean(),
  tags: z.array(z.string()),
  createdAt: z.iso.datetime(),
  updatedAt: z.iso.datetime().optional(),
});

const Comment = z.object({
  id: z.string().uuid(),
  postId: z.string().uuid(),
  authorId: z.string().uuid(),
  content: z.string().min(1).max(1000),
  createdAt: z.iso.datetime(),
});

// ============ Runtime Validation ============

// You can still use these schemas for runtime validation
const userData = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  email: "alice@example.com",
  name: "Alice",
  role: "author" as const,
  createdAt: new Date().toISOString(), // ISO 8601 string
};

const validatedUser = User.parse(userData);
console.log("Validated user:", validatedUser);

// ============ ODK Schema Creation ============

const blogSchema = odk.schema("BlogAPI", {
  UserRole,
  User,
  Post,
  Comment,
});

// ============ Output TypeSpec ============

const typespecSource = odk.toTypeSpec(blogSchema);
console.log("\n=== Generated TypeSpec ===\n");
console.log(typespecSource);

// ============ Output JSON (for Rust CLI) ============

const jsonSchema = odk.toJSON(blogSchema);
console.log("\n=== Compiled Schema JSON ===\n");
console.log(jsonSchema);

// ============ Emit to OpenAPI (uncomment to run) ============

// await odk.emit(blogSchema, {
//   targets: ["@typespec/openapi3"],
//   outDir: "./generated",
// });
// console.log("\nEmitted to ./generated/");
