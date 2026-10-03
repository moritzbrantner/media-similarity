import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { validateQualityRelationships } from "./quality-metrics.mjs";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const fixtureRoot = path.join(repoRoot, "tests/fixtures/quality-corpus/asset-tooling");

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function writeJson(file, value) {
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, `${JSON.stringify(value, null, 2)}\n`);
}

function git(root, args) {
  return execFileSync("git", ["-C", root, ...args], { encoding: "utf8" }).trim();
}

async function verifyAssetToolingCheckout(root, contract) {
  const packageJson = await readJson(path.join(root, "package.json"));
  if (packageJson.name !== "asset-tooling") throw new Error(`${root} is not an asset-tooling checkout`);
  const revision = git(root, ["rev-parse", "HEAD"]);
  if (revision !== contract.revision) {
    throw new Error(`asset-tooling revision mismatch: expected ${contract.revision}, got ${revision}`);
  }
  if (git(root, ["status", "--porcelain", "--untracked-files=no"]).length > 0) {
    throw new Error("asset-tooling checkout has tracked modifications; audio quality generation fails closed");
  }
  for (const key of ["store", "audioOperations"]) {
    const subpath = contract.exports?.[key];
    if (typeof subpath !== "string" || typeof packageJson.exports?.[subpath] !== "string") {
      throw new Error(`asset-tooling contract export '${key}' is unavailable`);
    }
  }
  return packageJson;
}

async function importExport(root, packageJson, subpath, revision) {
  return import(`${pathToFileURL(path.resolve(root, packageJson.exports[subpath])).href}?revision=${revision}`);
}

function validateAudioDefinitions(recipes, relationshipsValue) {
  if (!Array.isArray(recipes.audioSources) || recipes.audioSources.length < 2) {
    throw new Error("audio quality recipes need at least two audioSources");
  }
  if (!Array.isArray(recipes.audioRecipes) || recipes.audioRecipes.length === 0) {
    throw new Error("audio quality recipes need audioRecipes");
  }
  if (!Array.isArray(recipes.audioQueries) || recipes.audioQueries.length === 0) {
    throw new Error("audio quality recipes need audioQueries");
  }
  const sourceIds = new Set(recipes.audioSources.map((entry) => entry.assetId));
  const recipeIds = new Set(recipes.audioRecipes.map((entry) => entry.id));
  if (sourceIds.size !== recipes.audioSources.length) throw new Error("audioSources contain duplicate asset ids");
  if (recipeIds.size !== recipes.audioRecipes.length) throw new Error("audioRecipes contain duplicate ids");
  for (const query of recipes.audioQueries) {
    if (!sourceIds.has(query.sourceAssetId)) throw new Error(`audio query '${query.assetId}' references unknown source`);
    if (!recipeIds.has(query.recipeId)) throw new Error(`audio query '${query.assetId}' references unknown recipe`);
    if (!query.filename.endsWith(".wav")) throw new Error(`audio query '${query.assetId}' must materialize as WAV`);
  }
  const relationships = validateQualityRelationships(relationshipsValue);
  const queryIds = new Set(recipes.audioQueries.map((entry) => entry.assetId));
  const searchIds = new Set(recipes.audioQueries.map((entry) => `${entry.assetId}-search`));
  for (const relationship of relationships.searches) {
    if (!searchIds.has(relationship.searchId)) throw new Error(`unknown audio relationship search '${relationship.searchId}'`);
    if (!queryIds.has(relationship.queryAssetId)) throw new Error(`unknown audio relationship query '${relationship.queryAssetId}'`);
    for (const relevant of relationship.relevance) {
      if (!sourceIds.has(relevant.assetId)) throw new Error(`unknown audio relevant asset '${relevant.assetId}'`);
    }
    for (const negative of relationship.hardNegatives) {
      if (!sourceIds.has(negative)) throw new Error(`unknown audio hard negative '${negative}'`);
    }
  }
  return relationships;
}

async function writeAsset(storeModule, root, asset, filename) {
  const bytes = await storeModule.resolveAssetObject(root, asset);
  await mkdir(path.dirname(filename), { recursive: true });
  await writeFile(filename, bytes);
}

async function executeLinearRecipe(audio, root, source, recipe) {
  const handlers = {
    "audio.gain": [audio.createAudioGainOperationBuildIdentity, audio.executeAudioGainOperation],
    "audio.trim": [audio.createAudioTrimOperationBuildIdentity, audio.executeAudioTrimOperation],
    "audio.resample": [audio.createAudioResampleOperationBuildIdentity, audio.executeAudioResampleOperation],
  };
  let current = source;
  const steps = [];
  for (const step of recipe.steps) {
    const handler = handlers[step.operation];
    if (!handler) throw new Error(`unsupported audio quality operation '${step.operation}'`);
    const invocation = { parameters: step.parameters, inputs: { source: current } };
    const build = await handler[0](invocation);
    const result = await handler[1](root, invocation);
    steps.push({ operation: step.operation, buildIdentity: build, output: result.outputs.output, observations: result.observations });
    current = result.outputs.output;
  }
  return { output: current, steps };
}

async function executeAudioRecipe(audio, root, source, recipe) {
  if (Array.isArray(recipe.steps)) return executeLinearRecipe(audio, root, source, recipe);
  if (!recipe.noise || !recipe.mix) throw new Error(`audio recipe '${recipe.id}' has no executable definition`);
  const noiseInvocation = { parameters: recipe.noise, inputs: {} };
  const noiseBuild = await audio.createAudioSynthesizeOperationBuildIdentity(noiseInvocation);
  const noise = await audio.executeAudioSynthesizeOperation(root, noiseInvocation);
  const mixInvocation = {
    parameters: recipe.mix,
    inputs: { sources: [source, noise.outputs.output] },
  };
  const mixBuild = await audio.createAudioMixOperationBuildIdentity(mixInvocation);
  const mixed = await audio.executeAudioMixOperation(root, mixInvocation);
  return {
    output: mixed.outputs.output,
    steps: [
      { operation: "audio.synthesize", buildIdentity: noiseBuild, output: noise.outputs.output, observations: noise.observations },
      { operation: "audio.mix", buildIdentity: mixBuild, output: mixed.outputs.output, observations: mixed.observations },
    ],
  };
}

function patchCompatibilityManifest(manifest, recipes, relationships, contract) {
  const sourceById = new Map(recipes.audioSources.map((entry) => [entry.assetId, entry]));
  const relationshipById = new Map(relationships.searches.map((entry) => [entry.searchId, entry]));
  const provenanceUrl = `https://github.com/${contract.repository}/blob/${contract.revision}/docs/media-quality-fixtures.md`;
  const commitUrl = `https://github.com/${contract.repository}/commit/${contract.revision}`;

  for (const source of recipes.audioSources) {
    manifest.assets.push({
      id: source.assetId,
      kind: "audio",
      role: "source",
      filename: source.filename,
      title: source.title,
      identity: source.identity,
      download_url: commitUrl,
      page_url: provenanceUrl,
      license: "deterministic synthetic fixture",
      attribution: "Generated deterministically by asset-tooling",
    });
  }
  for (const query of recipes.audioQueries) {
    const source = sourceById.get(query.sourceAssetId);
    const searchId = `${query.assetId}-search`;
    const relationship = relationshipById.get(searchId);
    manifest.assets.push({
      id: query.assetId,
      kind: "audio",
      role: "query",
      filename: query.filename,
      title: query.title,
      identity: source.identity,
      copy_of: source.assetId,
      derivation: { type: "audio_reencode", start_seconds: 0, duration_seconds: 1, volume: 1 },
      expected_top_match: source.assetId,
      expected_top_k: relationship.relevance.map((entry) => entry.assetId),
      expected_non_matches: relationship.hardNegatives,
      capability: query.capability,
    });
    manifest.searches.push({
      id: searchId,
      query_asset: query.assetId,
      expected_identity: source.identity,
      expected_top_match: source.assetId,
      expected_top_k: relationship.relevance.map((entry) => entry.assetId),
      expected_non_matches: relationship.hardNegatives,
      capability: query.capability,
    });
  }
  manifest.name = `${manifest.name}+deterministic-audio-v1`;
  manifest.description = `${manifest.description} Includes deterministic synthesized audio robustness cases.`;
  return manifest;
}

async function main() {
  const contract = await readJson(path.join(fixtureRoot, "contract.json"));
  const recipes = await readJson(path.join(fixtureRoot, "recipes.json"));
  const relationshipsValue = await readJson(path.join(fixtureRoot, "audio-relationships.json"));
  const relationships = validateAudioDefinitions(recipes, relationshipsValue);
  if (process.argv.includes("--check")) {
    process.stdout.write(`asset-tooling audio quality contract is valid: ${recipes.audioSources.length} sources, ${recipes.audioQueries.length} queries\n`);
    return;
  }

  const outputIndex = process.argv.indexOf("--output");
  const outputDir = outputIndex >= 0
    ? path.resolve(repoRoot, process.argv[outputIndex + 1])
    : path.resolve(repoRoot, recipes.outputDir);
  const compatibilityPath = path.join(outputDir, ".asset-tooling-evaluator-root/tests/fixtures/quality-corpus/manifest.json");
  let compatibility;
  try {
    compatibility = await readJson(compatibilityPath);
  } catch {
    throw new Error("image quality materialization must run before audio quality materialization");
  }

  const assetToolingRoot = path.resolve(process.env.ASSET_TOOLING_ROOT ?? path.join(repoRoot, contract.defaultSibling));
  const packageJson = await verifyAssetToolingCheckout(assetToolingRoot, contract);
  const store = await importExport(assetToolingRoot, packageJson, contract.exports.store, contract.revision);
  const audio = await importExport(assetToolingRoot, packageJson, contract.exports.audioOperations, contract.revision);
  const toolRoot = path.join(outputDir, ".asset-tooling");

  const sourceAssets = new Map();
  const sourceEvidence = [];
  for (const source of recipes.audioSources) {
    const invocation = { parameters: source.parameters, inputs: {} };
    const build = await audio.createAudioSynthesizeOperationBuildIdentity(invocation);
    const result = await audio.executeAudioSynthesizeOperation(toolRoot, invocation);
    sourceAssets.set(source.assetId, result.outputs.output);
    await writeAsset(store, toolRoot, result.outputs.output, path.join(outputDir, source.filename));
    sourceEvidence.push({ assetId: source.assetId, buildIdentity: build, output: result.outputs.output, observations: result.observations });
  }

  const recipesById = new Map(recipes.audioRecipes.map((entry) => [entry.id, entry]));
  const queryEvidence = [];
  for (const query of recipes.audioQueries) {
    const source = sourceAssets.get(query.sourceAssetId);
    const recipe = recipesById.get(query.recipeId);
    const execution = await executeAudioRecipe(audio, toolRoot, source, recipe);
    await writeAsset(store, toolRoot, execution.output, path.join(outputDir, query.filename));
    queryEvidence.push({
      assetId: query.assetId,
      sourceAssetId: query.sourceAssetId,
      recipeId: query.recipeId,
      family: recipe.family,
      output: execution.output,
      steps: execution.steps,
    });
  }

  await writeJson(path.join(outputDir, "asset-tooling-audio-provenance.json"), {
    schemaVersion: 1,
    generator: { repository: contract.repository, revision: contract.revision },
    sources: sourceEvidence,
    queries: queryEvidence,
  });
  await writeJson(compatibilityPath, patchCompatibilityManifest(compatibility, recipes, relationships, contract));
  process.stdout.write(`asset-tooling audio quality corpus materialized: ${sourceEvidence.length} sources, ${queryEvidence.length} queries\n`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
