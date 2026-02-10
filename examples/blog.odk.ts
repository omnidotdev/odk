/**
 * Blog API Schema - TypeScript Format
 *
 * This example demonstrates defining a blog API schema using ODK's TypeScript SDK
 * with Zod for schema validation.
 *
 * Run with: bun run examples/blog.odk.ts
 */

import { odk } from "../odk-ts/src/index.js";
import { z } from "zod";

// Define enums as Zod enums
const UserRole = z.enum(["admin", "author", "reader"]);
const PostStatus = z.enum(["draft", "published", "archived"]);

// Define the User model
const User = z.object({
  id: z.string().uuid().describe("Unique user identifier"),
  email: z.string().email().describe("User's email address"),
  username: z
    .string()
    .min(3)
    .max(32)
    .regex(/^[a-zA-Z0-9_]+$/)
    .describe("Unique username"),
  displayName: z
    .string()
    .max(100)
    .optional()
    .describe("Display name shown on posts"),
  role: UserRole.describe("User's permission level"),
  createdAt: z.date(),
  updatedAt: z.date(),
});

// Define the Post model
const Post = z.object({
  id: z.string().uuid(),
  title: z.string().min(1).max(200).describe("Post title"),
  slug: z
    .string()
    .regex(/^[a-z0-9-]+$/)
    .describe("URL-friendly slug"),
  content: z.string().describe("Post content in Markdown"),
  excerpt: z.string().max(500).optional().describe("Short excerpt for previews"),
  authorId: z.string().uuid(),
  status: PostStatus,
  publishedAt: z.date().optional(),
  createdAt: z.date(),
  updatedAt: z.date(),
});

// Define the Comment model
const Comment = z.object({
  id: z.string().uuid(),
  content: z.string().min(1).max(2000),
  postId: z.string().uuid(),
  authorId: z.string().uuid(),
  parentId: z
    .string()
    .uuid()
    .optional()
    .describe("Parent comment ID for nested replies"),
  createdAt: z.date(),
});

// Define the Tag model
const Tag = z.object({
  id: z.string().uuid(),
  name: z.string().min(1).max(50),
  slug: z.string().regex(/^[a-z0-9-]+$/),
});

// Create the ODK schema
const schema = odk.schema(
  "BlogAPI",
  {
    User,
    Post,
    Comment,
    Tag,
    UserRole,
    PostStatus,
  },
  {
    imports: ["@typespec/http", "@omnidotdev/typespec-drizzle"],
    description: "Blog API with users, posts, comments, and tags",
  }
);

// Add raw TypeSpec for API routes (escape hatch for things ODK doesn't model)
odk.raw(
  schema,
  `
  @route("/api")
  @tag("Blog API")
  namespace BlogApiRoutes {
    @route("/posts")
    interface Posts {
      @get list(@query status?: PostStatus): Post[];
      @get read(@path slug: string): Post;
      @post create(@body post: Post): Post;
      @put update(@path id: string, @body post: Post): Post;
      @delete remove(@path id: string): void;
    }

    @route("/users")
    interface Users {
      @get list(): User[];
      @get read(@path id: string): User;
      @post create(@body user: User): User;
    }

    @route("/posts/{postId}/comments")
    interface Comments {
      @get list(@path postId: string): Comment[];
      @post create(@path postId: string, @body comment: Comment): Comment;
    }
  }
`,
  "API route definitions"
);

// Output options:

// 1. Get TypeSpec source code
const typespec = odk.toTypeSpec(schema);
console.log("=== Generated TypeSpec ===");
console.log(typespec);

// 2. Get JSON for Rust CLI
const json = odk.toJSON(schema);
console.log("\n=== Compiled Schema JSON ===");
console.log(json);

// 3. Emit to targets (requires tspconfig.yaml)
// await odk.emit(schema, {
//   targets: ["@typespec/openapi3", "@typespec/protobuf"],
//   outDir: "./generated",
// });
