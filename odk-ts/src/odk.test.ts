import { describe, expect, test } from "bun:test";

import { z } from "zod";

import { odk } from "./index.js";

const User = z.object({
  id: z.string(),
  email: z.string(),
  name: z.string(),
});

describe("odk", () => {
  test("schema() compiles Standard Schema types into a named model", () => {
    const schema = odk.schema("TestAPI", { User });

    expect(schema.compiled.name).toBe("TestAPI");
    expect(schema.types.User).toBe(User);

    const user = schema.compiled.models.find((model) => model.name === "User");
    const fieldNames = user?.fields.map((field) => field.name) ?? [];
    expect(fieldNames).toContain("id");
    expect(fieldNames).toContain("email");
    expect(fieldNames).toContain("name");
  });

  test("toJSON() round-trips the compiled schema", () => {
    const schema = odk.schema("TestAPI", { User });

    expect(JSON.parse(odk.toJSON(schema))).toEqual(odk.getCompiled(schema));
  });

  test("toTypeSpec() renders the namespace and model", () => {
    const tsp = odk.toTypeSpec(odk.schema("TestAPI", { User }));

    expect(tsp).toContain("TestAPI");
    expect(tsp).toContain("User");
  });

  test("addImport() adds an import without duplicating it", () => {
    const schema = odk.schema("TestAPI", { User });
    odk.addImport(schema, "@typespec/http");
    odk.addImport(schema, "@typespec/http");

    expect(
      schema.compiled.imports?.filter((i) => i === "@typespec/http"),
    ).toEqual(["@typespec/http"]);
  });
});
