"""
Union class for ODK schemas.

Mirrors CompiledUnion from Rust odk-schema.
"""

from __future__ import annotations

from dataclasses import dataclass
from dataclasses import field as dataclass_field
from typing import Any

from .types import CompiledType, Extensions, SourceSpan


@dataclass
class UnionVariant:
    """A variant within a union."""

    name: str
    variant_type: CompiledType | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict."""
        result: dict[str, Any] = {"name": self.name}
        if self.variant_type:
            result["type"] = self.variant_type.to_dict()
        return result


@dataclass
class Union:
    """
    A union/enum type in the schema.

    For simple string enums, just pass a list of strings as variants.
    For discriminated unions, pass UnionVariant objects.

    Examples:
        # Simple string enum
        UserRole = Union(
            "UserRole",
            variants=["admin", "author", "reader"],
            description="User permission levels",
        )

        # Discriminated union
        SearchResult = Union(
            "SearchResult",
            typed_variants=[
                UnionVariant("user", CompiledType.ref("User")),
                UnionVariant("post", CompiledType.ref("Post")),
            ],
        )
    """

    name: str
    variants: list[str] | None = None
    typed_variants: list[UnionVariant] | None = None
    description: str | None = None
    decorators: list[str] | None = None
    source: SourceSpan | None = None

    def get_variants(self) -> list[UnionVariant]:
        """Get all variants as UnionVariant objects."""
        if self.typed_variants:
            return self.typed_variants

        if self.variants:
            # Convert string variants to literal type variants
            return [
                UnionVariant(name=v, variant_type=CompiledType.literal_string(v))
                for v in self.variants
            ]

        return []

    def get_extensions(self) -> Extensions:
        """Get extensions including decorators."""
        extensions = Extensions()
        if self.decorators:
            for decorator in self.decorators:
                extensions.add_decorator(decorator)
        return extensions

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict matching Rust CompiledUnion JSON serialization."""
        result: dict[str, Any] = {
            "name": self.name,
            "variants": [v.to_dict() for v in self.get_variants()],
        }

        if self.description:
            result["description"] = self.description

        extensions = self.get_extensions()
        if not extensions.is_empty():
            result["extensions"] = extensions.to_dict()

        if self.source:
            result["source"] = self.source.to_dict()

        return result
