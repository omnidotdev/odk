/**
 * Blog API Schema - TypeScript Format
 *
 * This example demonstrates defining a blog API schema using ODK's TypeScript SDK
 * with Zod for schema validation.
 *
 * Compile with: odk compile examples/blog.odk.ts -o generated/
 */

import { odk } from "@omnidotdev/odk";
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
  createdAt: z.iso.datetime(),
  updatedAt: z.iso.datetime(),
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
  excerpt: z
    .string()
    .max(500)
    .optional()
    .describe("Short excerpt for previews"),
  authorId: z.string().uuid(),
  status: PostStatus,
  publishedAt: z.iso.datetime().optional(),
  createdAt: z.iso.datetime(),
  updatedAt: z.iso.datetime(),
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
  createdAt: z.iso.datetime(),
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
    imports: ["@typespec/http"],
    description: "Blog API with users, posts, comments, and tags",
  },
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
  "API route definitions",
);

// Export the schema so `odk compile blog.odk.ts` can load and compile it. The
// CLI serializes this default export's compiled IR, so the module must not
// print to stdout itself
export default schema;
