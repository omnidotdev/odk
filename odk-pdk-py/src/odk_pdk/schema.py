"""
Schema class for ODK schemas.

Mirrors CompiledSchema from Rust odk-schema.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from dataclasses import field as dataclass_field
from typing import Any

from .model import Model
from .types import CompiledOperation, RawBlock
from .union import Union


@dataclass
class Schema:
    """
    The root schema container.

    Examples:
        schema = Schema("BlogAPI")
        schema.add_import("@typespec/http")
        schema.add_model(User)
        schema.add_union(UserRole)
        schema.add_raw('''
            @route("/api")
            interface Api { ... }
        ''')

        # Export as JSON for WASM
        json_str = schema.to_json()
    """

    name: str
    models: list[Model] = dataclass_field(default_factory=list)
    unions: list[Union] = dataclass_field(default_factory=list)
    operations: list[CompiledOperation] = dataclass_field(default_factory=list)
    imports: list[str] = dataclass_field(default_factory=list)
    raw_blocks: list[RawBlock] = dataclass_field(default_factory=list)
    warnings: list[str] | None = None

    def add_model(self, model: Model) -> None:
        """Add a model to the schema."""
        self.models.append(model)

    def add_models(self, models: list[Model]) -> None:
        """Add multiple models to the schema."""
        self.models.extend(models)

    def add_union(self, union: Union) -> None:
        """Add a union to the schema."""
        self.unions.append(union)

    def add_unions(self, unions: list[Union]) -> None:
        """Add multiple unions to the schema."""
        self.unions.extend(unions)

    def add_operation(self, operation: CompiledOperation) -> None:
        """Add an operation to the schema."""
        self.operations.append(operation)

    def add_operations(self, operations: list[CompiledOperation]) -> None:
        """Add multiple operations to the schema."""
        self.operations.extend(operations)

    def add_import(self, import_path: str) -> None:
        """Add a TypeSpec import."""
        if import_path not in self.imports:
            self.imports.append(import_path)

    def add_raw(self, content: str, description: str | None = None) -> None:
        """Add a raw TypeSpec block."""
        self.raw_blocks.append(RawBlock(content=content, description=description))

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict matching Rust CompiledSchema JSON serialization."""
        result: dict[str, Any] = {
            "name": self.name,
            "models": [m.to_dict() for m in self.models],
            "unions": [u.to_dict() for u in self.unions],
        }

        if self.operations:
            result["operations"] = [o.to_dict() for o in self.operations]

        if self.raw_blocks:
            result["raw_blocks"] = [r.to_dict() for r in self.raw_blocks]

        if self.imports:
            result["imports"] = self.imports

        if self.warnings:
            result["warnings"] = self.warnings

        return result

    def to_json(self) -> str:
        """Serialize to JSON string for Extism/WASM export."""
        return json.dumps(self.to_dict(), indent=2)

    @classmethod
    def from_json(cls, json_str: str) -> Schema:
        """Deserialize from JSON string."""
        data = json.loads(json_str)
        # TODO: Full deserialization if needed
        return cls(name=data["name"])
