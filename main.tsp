import "@typespec/http";
import "@typespec/protobuf";

// Blog schema focused on real use case

// Status enums
union UserRole {
  admin: "admin",
  editor: "editor",
  author: "author",
  subscriber: "subscriber",
}

union PostStatus {
  draft: "draft",
  published: "published",
  archived: "archived",
}

union CommentStatus {
  pending: "pending",
  approved: "approved",
  rejected: "rejected",
}

// Core blog models
model User {
  id: string;
  email: string;
  username: string;
  firstName?: string;
  lastName?: string;
  role: UserRole;
  bio?: string;
  avatarUrl?: string;
  isActive: boolean;
  emailVerified: boolean;
  createdAt: string;
  updatedAt?: string;
}

model Category {
  id: int32;
  name: string;
  slug: string;
  description?: string;
  parentId?: int32;
  isActive: boolean;
  createdAt: string;
}

model Tag {
  id: int32;
  name: string;
  slug: string;
  description?: string;
  color?: string;
  createdAt: string;
}

model Post {
  id: int64;
  title: string;
  slug: string;
  excerpt?: string;
  content?: string;
  status: PostStatus;
  authorId: string;
  categoryId?: int32;
  featuredImageUrl?: string;
  publishedAt?: string;
  createdAt: string;
  updatedAt?: string;
  viewCount: int32;
  metadata?: {};
  tags?: string[];
}

model Comment {
  id: int64;
  postId: int64;
  authorId?: string;
  authorName?: string;
  authorEmail?: string;
  content: string;
  status: CommentStatus;
  parentId?: int64;
  ipAddress?: string;
  userAgent?: string;
  createdAt: string;
  updatedAt?: string;
}

model PostTag {
  postId: int64;
  tagId: int32;
  createdAt: string;
}

// Analytics models
model PageView {
  id: int64;
  postId?: int64;
  userId?: string;
  ipAddress: string;
  userAgent: string;
  referer?: string;
  path: string;
  timestamp: string;
}

model UserSession {
  id: string;
  userId: string;
  ipAddress: string;
  userAgent: string;
  loginAt: string;
  lastActivityAt: string;
  logoutAt?: string;
  isActive: boolean;
}

// Configuration model
model SiteSetting {
  key: string;
  value: string;
  description?: string;
  type: string;
  createdAt: string;
  updatedAt?: string;
}
