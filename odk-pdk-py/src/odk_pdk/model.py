"""
Model class for ODK schemas.

Mirrors CompiledModel from Rust odk-schema.
"""

from __future__ import annotations

from dataclasses import dataclass
from dataclasses import field as dataclass_field
from typing import Any

from .field import Field
from .types import Extensions, SourceSpan


@dataclass
class Model:
    """
    A model (object type) in the schema.

    Examples:
        User = Model(
            "User",
            description="A user account",
            decorators=["@table"],
            fields=[
                Field("id", type="string", format="uuid", decorators=["@id"]),
                Field("email", type="string", format="email"),
                Field("name", type="string", optional=True),
            ],
        )
    """

    name: str
    fields: list[Field] = dataclass_field(default_factory=list)
    description: str | None = None
    decorators: list[str] | None = None
    source: SourceSpan | None = None

    def add_field(self, field: Field) -> None:
        """Add a field to the model."""
        self.fields.append(field)

    def get_extensions(self) -> Extensions:
        """Get extensions including decorators."""
        extensions = Extensions()
        if self.decorators:
            for decorator in self.decorators:
                extensions.add_decorator(decorator)
        return extensions

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict matching Rust CompiledModel JSON serialization."""
        result: dict[str, Any] = {
            "name": self.name,
            "fields": [f.to_dict() for f in self.fields],
        }

        if self.description:
            result["description"] = self.description

        extensions = self.get_extensions()
        if not extensions.is_empty():
            result["extensions"] = extensions.to_dict()

        if self.source:
            result["source"] = self.source.to_dict()

        return result
