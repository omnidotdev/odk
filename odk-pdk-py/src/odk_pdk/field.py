"""
Field class for ODK schemas.

Mirrors CompiledField from Rust odk-schema.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from .types import CompiledType, Extensions, FieldConstraints, SourceSpan


@dataclass
class Field:
    """
    A field within a model.

    Can be created with either a scalar type or a reference to another type.

    Examples:
        # Scalar type with format
        Field("id", type="string", format="uuid")

        # Reference to another type
        Field("status", ref="PostStatus")

        # Array of references
        Field("posts", ref="Post", array=True)

        # With constraints
        Field("title", type="string", min_length=1, max_length=200)

        # With decorators
        Field("id", type="string", decorators=["@id", "@key"])
    """

    name: str
    type: str | None = None
    ref: str | None = None
    array: bool = False
    optional: bool = False
    description: str | None = None
    decorators: list[str] | None = None
    # Constraints
    format: str | None = None
    min_length: int | None = None
    max_length: int | None = None
    min_value: float | None = None
    max_value: float | None = None
    pattern: str | None = None
    multiple_of: float | None = None
    # Source mapping
    source: SourceSpan | None = None

    def get_compiled_type(self) -> CompiledType:
        """Get the CompiledType for this field."""
        if self.ref:
            base_type = CompiledType.ref(self.ref)
        elif self.type:
            base_type = CompiledType.scalar(self.type)
        else:
            raise ValueError(f"Field '{self.name}' must have either 'type' or 'ref'")

        if self.array:
            return CompiledType.array(base_type)
        return base_type

    def get_constraints(self) -> FieldConstraints | None:
        """Get field constraints if any are set."""
        constraints = FieldConstraints(
            min_length=self.min_length,
            max_length=self.max_length,
            min_value=self.min_value,
            max_value=self.max_value,
            pattern=self.pattern,
            format=self.format,
            multiple_of=self.multiple_of,
        )
        # Return None if all constraints are None
        if all(
            v is None
            for v in [
                constraints.min_length,
                constraints.max_length,
                constraints.min_value,
                constraints.max_value,
                constraints.pattern,
                constraints.format,
                constraints.multiple_of,
            ]
        ):
            return None
        return constraints

    def get_extensions(self) -> Extensions:
        """Get extensions including decorators."""
        extensions = Extensions()
        if self.decorators:
            for decorator in self.decorators:
                extensions.add_decorator(decorator)
        return extensions

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict matching Rust CompiledField JSON serialization."""
        result: dict[str, Any] = {
            "name": self.name,
            "type": self.get_compiled_type().to_dict(),
        }

        if self.optional:
            result["optional"] = True

        if self.description:
            result["description"] = self.description

        constraints = self.get_constraints()
        if constraints:
            result["constraints"] = constraints.to_dict()

        extensions = self.get_extensions()
        if not extensions.is_empty():
            result["extensions"] = extensions.to_dict()

        if self.source:
            result["source"] = self.source.to_dict()

        return result
