#!/usr/bin/env bun
/**
 * Propagate the release version to every ODK artifact.
 *
 * The source of truth is `odk-ts/package.json` (bumped by `changeset version`).
 * This script writes that version into the Rust crates and the Python PDK so a
 * single release ships the npm SDK, the crates, and the PyPI package in lockstep.
 *
 * Run via `bun run sync:versions`, invoked by the `version` script right after
 * `changeset version`
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const version = (
  JSON.parse(readFileSync(join(root, "odk-ts/package.json"), "utf8")) as {
    version: string;
  }
).version;

if (!/^\d+\.\d+\.\d+/.test(version)) {
  throw new Error(`refusing to sync an unexpected version string: ${version}`);
}

/** Rewrite a crate's own `[package]` version and any intra-workspace path-dep requirement */
const syncCargo = (relPath: string) => {
  const path = join(root, relPath);
  let src = readFileSync(path, "utf8");
  // the crate's own version: the first line-start `version = "..."` ([package])
  src = src.replace(/^version = "[^"]*"/m, `version = "${version}"`);
  // the odk-schema path-dependency requirement, where present
  src = src.replace(
    /(path = "\.\.\/odk-schema", version = ")[^"]*(")/g,
    `$1${version}$2`,
  );
  writeFileSync(path, src);
};

for (const crate of [
  "odk-schema/Cargo.toml",
  "odk-cli/Cargo.toml",
  "odk-pdk-rust/Cargo.toml",
]) {
  syncCargo(crate);
}

// Python PDK
const pyPath = join(root, "odk-pdk-py/pyproject.toml");
writeFileSync(
  pyPath,
  readFileSync(pyPath, "utf8").replace(
    /^version = "[^"]*"/m,
    `version = "${version}"`,
  ),
);

// keep the private workspace root in lockstep for clarity
const rootPkgPath = join(root, "package.json");
const rootPkg = JSON.parse(readFileSync(rootPkgPath, "utf8"));
rootPkg.version = version;
writeFileSync(rootPkgPath, `${JSON.stringify(rootPkg, null, 2)}\n`);

process.stdout.write(
  `synced version ${version} to crates, python PDK, and workspace root\n`,
);
