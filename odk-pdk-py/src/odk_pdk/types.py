"""
Type definitions for ODK schemas.

These mirror the Rust CompiledType and ScalarType enums from odk-schema.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class ScalarType(Enum):
    """Scalar type names matching Rust ScalarType enum."""

    STRING = "string"
    INT32 = "int32"
    INT64 = "int64"
    FLOAT32 = "float32"
    FLOAT64 = "float64"
    BOOLEAN = "boolean"
    DATE = "date"
    DATETIME = "datetime"
    UTC_DATETIME = "utcDateTime"
    OFFSET_DATETIME = "offsetDateTime"
    PLAIN_DATE = "plainDate"
    PLAIN_TIME = "plainTime"
    DURATION = "duration"
    BYTES = "bytes"
    URL = "url"
    UNKNOWN = "unknown"


@dataclass
class FieldConstraints:
    """Field validation constraints."""

    min_length: int | None = None
    max_length: int | None = None
    min_value: float | None = None
    max_value: float | None = None
    pattern: str | None = None
    format: str | None = None
    multiple_of: float | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict, omitting None values."""
        result = {}
        if self.min_length is not None:
            result["minLength"] = self.min_length
        if self.max_length is not None:
            result["maxLength"] = self.max_length
        if self.min_value is not None:
            result["minValue"] = self.min_value
        if self.max_value is not None:
            result["maxValue"] = self.max_value
        if self.pattern is not None:
            result["pattern"] = self.pattern
        if self.format is not None:
            result["format"] = self.format
        if self.multiple_of is not None:
            result["multipleOf"] = self.multiple_of
        return result


@dataclass
class CompiledType:
    """
    A type reference. Uses tagged union pattern matching Rust serde output.

    Kind can be: scalar, array, ref, literal, union, object
    """

    kind: str
    # For scalar
    name: str | None = None
    # For array
    items: CompiledType | None = None
    # For literal
    value: str | int | float | bool | None = None
    # For union
    variants: list[CompiledType] | None = None
    # For object
    fields: list[Any] | None = None  # list[CompiledField] - forward ref

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict matching Rust JSON serialization."""
        result: dict[str, Any] = {"kind": self.kind}

        if self.kind == "scalar":
            result["name"] = self.name
        elif self.kind == "array":
            result["items"] = self.items.to_dict() if self.items else None
        elif self.kind == "ref":
            result["name"] = self.name
        elif self.kind == "literal":
            result["value"] = self.value
        elif self.kind == "union":
            result["variants"] = (
                [v.to_dict() for v in self.variants] if self.variants else []
            )
        elif self.kind == "object":
            result["fields"] = [f.to_dict() for f in self.fields] if self.fields else []

        return result

    @classmethod
    def string(cls) -> CompiledType:
        """Create a string type."""
        return cls(kind="scalar", name="string")

    @classmethod
    def int32(cls) -> CompiledType:
        """Create an int32 type."""
        return cls(kind="scalar", name="int32")

    @classmethod
    def int64(cls) -> CompiledType:
        """Create an int64 type."""
        return cls(kind="scalar", name="int64")

    @classmethod
    def float32(cls) -> CompiledType:
        """Create a float32 type."""
        return cls(kind="scalar", name="float32")

    @classmethod
    def float64(cls) -> CompiledType:
        """Create a float64 type."""
        return cls(kind="scalar", name="float64")

    @classmethod
    def boolean(cls) -> CompiledType:
        """Create a boolean type."""
        return cls(kind="scalar", name="boolean")

    @classmethod
    def datetime(cls) -> CompiledType:
        """Create a datetime type."""
        return cls(kind="scalar", name="datetime")

    @classmethod
    def date(cls) -> CompiledType:
        """Create a date type."""
        return cls(kind="scalar", name="date")

    @classmethod
    def bytes(cls) -> CompiledType:
        """Create a bytes type."""
        return cls(kind="scalar", name="bytes")

    @classmethod
    def scalar(cls, name: str) -> CompiledType:
        """Create a scalar type by name."""
        return cls(kind="scalar", name=name)

    @classmethod
    def array(cls, items: CompiledType) -> CompiledType:
        """Create an array type."""
        return cls(kind="array", items=items)

    @classmethod
    def ref(cls, name: str) -> CompiledType:
        """Create a reference to another type."""
        return cls(kind="ref", name=name)

    @classmethod
    def literal_string(cls, value: str) -> CompiledType:
        """Create a string literal type."""
        return cls(kind="literal", value=value)

    @classmethod
    def literal_number(cls, value: int | float) -> CompiledType:
        """Create a number literal type."""
        return cls(kind="literal", value=value)

    @classmethod
    def literal_bool(cls, value: bool) -> CompiledType:
        """Create a boolean literal type."""
        return cls(kind="literal", value=value)


@dataclass
class Extensions:
    """Extensions for decorators and custom metadata."""

    known: list[dict[str, Any]] = field(default_factory=list)
    unknown: dict[str, Any] = field(default_factory=dict)

    def add_decorator(self, value: str) -> None:
        """Add a decorator extension."""
        self.known.append({"kind": "decorator", "value": value})

    def is_empty(self) -> bool:
        """Check if extensions are empty."""
        return len(self.known) == 0 and len(self.unknown) == 0

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict, omitting if empty."""
        if self.is_empty():
            return {}
        result: dict[str, Any] = {}
        if self.known:
            result["known"] = self.known
        if self.unknown:
            result.update(self.unknown)
        return result


@dataclass
class SourceSpan:
    """Source location for error mapping."""

    file: str
    line: int
    column: int
    end_line: int | None = None
    end_column: int | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict."""
        result = {"file": self.file, "line": self.line, "column": self.column}
        if self.end_line is not None:
            result["endLine"] = self.end_line
        if self.end_column is not None:
            result["endColumn"] = self.end_column
        return result


@dataclass
class RawBlock:
    """Raw TypeSpec block for escape hatch."""

    content: str
    description: str | None = None
    source: SourceSpan | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict."""
        result: dict[str, Any] = {"content": self.content}
        if self.description:
            result["description"] = self.description
        if self.source:
            result["source"] = self.source.to_dict()
        return result
