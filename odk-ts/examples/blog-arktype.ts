/**
 * Example: Blog API with ArkType
 *
 * This example demonstrates using ODK with ArkType schemas to:
 * 1. Define runtime-validated types
 * 2. Generate TypeSpec source
 * 3. Emit to OpenAPI (and other targets)
 */

import { odk } from "../src/index.js";
import { type } from "arktype";

// ============ Schema Definitions ============

const UserRole = type("'admin' | 'author' | 'reader'");

const User = type({
  id: "string.uuid",
  email: "string.email",
  name: "1 < string <= 100", // String with length constraints
  role: "'admin' | 'author' | 'reader'",
  createdAt: "string.date.iso", // ISO date string
  "bio?": "string <= 500", // Optional with max length
});

const Post = type({
  id: "string.uuid",
  title: "1 < string <= 200",
  slug: "/^[a-z0-9-]+$/", // Regex pattern
  content: "string",
  authorId: "string.uuid",
  published: "boolean",
  tags: "string[]",
  createdAt: "string.date.iso",
  "updatedAt?": "string.date.iso",
});

const Comment = type({
  id: "string.uuid",
  postId: "string.uuid",
  authorId: "string.uuid",
  content: "1 < string <= 1000",
  createdAt: "string.date.iso",
});

// ============ Runtime Validation ============

// You can still use these schemas for runtime validation
const userData = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  email: "alice@example.com",
  name: "Alice",
  role: "author" as const,
  createdAt: new Date().toISOString(),
};

const result = User(userData);
if (result instanceof type.errors) {
  console.log("Validation errors:", result.summary);
} else {
  console.log("Validated user:", result);
}

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
