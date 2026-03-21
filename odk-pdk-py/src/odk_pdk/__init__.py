"""
ODK Plugin Development Kit for Python.

Define schemas in Python, compile to WASM, run via ODK CLI.

Example:
    from odk_pdk import Schema, Model, Field, Union

    schema = Schema("MyAPI")

    User = Model(
        "User",
        fields=[
            Field("id", type="string", format="uuid"),
            Field("email", type="string", format="email"),
        ],
    )

    schema.add_model(User)

    # Export JSON for WASM
    print(schema.to_json())
"""

from .field import Field
from .model import Model
from .schema import Schema
from .types import (
    CompiledOperation,
    CompiledType,
    Directive,
    Extensions,
    FieldConstraints,
    FieldSelection,
    OperationKind,
    RawBlock,
    ScalarType,
    SelectionSet,
    SourceSpan,
)
from .union import Union, UnionVariant

__all__ = [
    # Core classes
    "Schema",
    "Model",
    "Field",
    "Union",
    "UnionVariant",
    # Type system
    "CompiledType",
    "ScalarType",
    "FieldConstraints",
    "Extensions",
    "SourceSpan",
    "RawBlock",
    # Operations
    "CompiledOperation",
    "OperationKind",
    "SelectionSet",
    "FieldSelection",
    "Directive",
]

__version__ = "0.1.0"
