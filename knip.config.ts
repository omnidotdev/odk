import type { KnipConfig } from "knip";

/**
 * Knip configuration.
 * @see https://knip.dev/overview/configuration
 */
const knipConfig: KnipConfig = {
  ignore: ["examples/**", "odk-ts/examples/**"],
  // odk-ts is a publishable library; exclude from unused-export analysis
  exclude: ["unresolved"],
  workspaces: {
    ".": {},
    "odk-ts": {
      entry: ["src/**/*.test.ts"],
      includeEntryExports: false,
      ignoreDependencies: ["arktype", "@valibot/to-json-schema"],
    },
  },
};

export default knipConfig;
