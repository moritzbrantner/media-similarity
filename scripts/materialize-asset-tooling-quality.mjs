import { execFileSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { validateQualityRelationships } from "./quality-metrics.mjs";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const fixtureRoot = path.join(repoRoot, "tests/fixtures/quality-corpus");
const assetFixtureRoot = path.join(fixtureRoot, "asset-tooling");

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function writeJson(file, value) {
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, `${JSON.stringify(value, null, 2)}\n`);
}

function requireString(value, location) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${location} must be a non-empty string`);
  }
  return value;
}

function assertGitRevision(value, location) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(`${location} must be a lowercase 40-character Git commit SHA`);
  }
  return value;
}

function assertSha256(value, location) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`${location} must be a lowercase SHA-256 digest`);
  }
  return value;
}

function uniqueBy(values, key, location) {
  const seen = new Set();
  for (const value of values) {
    const id = value[key];
    if (seen.has(id)) throw new Error(`${location} contains duplicate ${key} '${id}'`);
    seen.add(id);
  }
  return seen;
}

function validateFixtureDocuments(contract, catalogDocument, recipesDocument, relationshipsDocument, baseManifest) {
  if (contract.version !== 1) throw new Error("asset-tooling contract version must be 1");
  requireString(contract.repository, "asset-tooling contract repository");
  assertGitRevision(contract.revision, "asset-tooling contract revision");
  requireString(contract.defaultSibling, "asset-tooling contract defaultSibling");
  for (const key of ["catalog", "catalogAcquisition", "store", "imageCodecs", "imagePerturbations"]) {
    requireString(contract.exports?.[key], `asset-tooling contract exports.${key}`);
  }

  if (catalogDocument.version !== 1 || !Array.isArray(catalogDocument.providers) || !Array.isArray(catalogDocument.sources)) {
    throw new Error("asset-tooling quality catalog must contain version 1 providers and sources");
  }
  const catalogIds = uniqueBy(catalogDocument.sources, "id", "asset-tooling quality catalog sources");
  for (const source of catalogDocument.sources) {
    requireString(source.id, "catalog source id");
    requireString(source.source?.url, `catalog source '${source.id}' url`);
    assertSha256(source.source?.sha256, `catalog source '${source.id}' sha256`);
    if (!Number.isSafeInteger(source.source?.byteLength) || source.source.byteLength <= 0) {
      throw new Error(`catalog source '${source.id}' byteLength must be positive`);
    }
  }

  if (recipesDocument.version !== 1 || !Array.isArray(recipesDocument.sources) || !Array.isArray(recipesDocument.recipes) || !Array.isArray(recipesDocument.queries)) {
    throw new Error("asset-tooling quality recipes must contain version 1 sources, recipes, and queries");
  }
  const sourceAssetIds = uniqueBy(recipesDocument.sources, "assetId", "asset-tooling quality sources");
  const recipeIds = uniqueBy(recipesDocument.recipes, "id", "asset-tooling quality recipes");
  const queryAssetIds = uniqueBy(recipesDocument.queries, "assetId", "asset-tooling quality queries");
  for (const source of recipesDocument.sources) {
    if (!catalogIds.has(source.catalogSourceId)) {
      throw new Error(`quality source '${source.assetId}' references unknown catalog source '${source.catalogSourceId}'`);
    }
    requireString(source.filename, `quality source '${source.assetId}' filename`);
    requireString(source.identity, `quality source '${source.assetId}' identity`);
  }
  for (const query of recipesDocument.queries) {
    if (!sourceAssetIds.has(query.sourceAssetId)) {
      throw new Error(`quality query '${query.assetId}' references unknown source '${query.sourceAssetId}'`);
    }
    if (!recipeIds.has(query.recipeId)) {
      throw new Error(`quality query '${query.assetId}' references unknown recipe '${query.recipeId}'`);
    }
    requireString(query.filename, `quality query '${query.assetId}' filename`);
  }

  const relationships = validateQualityRelationships(relationshipsDocument);
  const knownAssetIds = new Set([
    ...baseManifest.assets.map((asset) => asset.id),
    ...sourceAssetIds,
    ...queryAssetIds,
  ]);
  const knownSearchIds = new Set([
    ...baseManifest.searches.map((search) => search.id),
    ...recipesDocument.queries.map((query) => `${query.assetId}-search`),
  ]);
  for (const relationship of relationships.searches) {
    if (!knownSearchIds.has(relationship.searchId)) {
      throw new Error(`relationship references unknown search '${relationship.searchId}'`);
    }
    if (!knownAssetIds.has(relationship.queryAssetId)) {
      throw new Error(`relationship search '${relationship.searchId}' references unknown query asset '${relationship.queryAssetId}'`);
    }
    for (const entry of relationship.relevance) {
      if (!knownAssetIds.has(entry.assetId)) {
        throw new Error(`relationship search '${relationship.searchId}' references unknown relevant asset '${entry.assetId}'`);
      }
    }
    for (const assetId of relationship.hardNegatives) {
      if (!knownAssetIds.has(assetId)) {
        throw new Error(`relationship search '${relationship.searchId}' references unknown hard negative '${assetId}'`);
      }
    }
  }

  return relationships;
}

function git(assetToolingRoot, args) {
  return execFileSync("git", ["-C", assetToolingRoot, ...args], { encoding: "utf8" }).trim();
}

async function verifyAssetToolingCheckout(assetToolingRoot, contract) {
  const packageJson = await readJson(path.join(assetToolingRoot, "package.json"));
  if (packageJson.name !== "asset-tooling") {
    throw new Error(`${assetToolingRoot} is not an asset-tooling checkout`);
  }
  const revision = git(assetToolingRoot, ["rev-parse", "HEAD"]);
  if (revision !== contract.revision) {
    throw new Error(`asset-tooling revision mismatch: expected ${contract.revision}, got ${revision}`);
  }
  const dirty = git(assetToolingRoot, ["status", "--porcelain", "--untracked-files=no"]);
  if (dirty.length > 0) {
    throw new Error("asset-tooling checkout has tracked modifications; quality generation fails closed");
  }
  for (const [name, subpath] of Object.entries(contract.exports)) {
    const target = packageJson.exports?.[subpath];
    if (typeof target !== "string") {
      throw new Error(`asset-tooling package does not export ${subpath} required by contract key '${name}'`);
    }
  }
  return packageJson;
}

async function importAssetToolingExport(assetToolingRoot, packageJson, subpath, revision) {
  const target = packageJson.exports[subpath];
  const resolved = path.resolve(assetToolingRoot, target);
  return import(`${pathToFileURL(resolved).href}?revision=${revision}`);
}

function sourceByAssetId(recipesDocument, assetId) {
  const source = recipesDocument.sources.find((entry) => entry.assetId === assetId);
  if (!source) throw new Error(`unknown source asset '${assetId}'`);
  return source;
}

function relationshipBySearchId(relationships, searchId) {
  const relationship = relationships.searches.find((entry) => entry.searchId === searchId);
  if (!relationship) throw new Error(`missing relationship for generated search '${searchId}'`);
  return relationship;
}

function catalogSourceById(catalogDocument, sourceId) {
  const source = catalogDocument.sources.find((entry) => entry.id === sourceId);
  if (!source) throw new Error(`missing catalog source '${sourceId}'`);
  return source;
}

function buildCompatibilityManifest(baseManifest, catalogDocument, recipesDocument, relationships) {
  const assets = [...baseManifest.assets];
  for (const source of recipesDocument.sources) {
    const catalogSource = catalogSourceById(catalogDocument, source.catalogSourceId);
    assets.push({
      id: source.assetId,
      kind: "static_image",
      role: "source",
      filename: source.filename,
      title: source.title,
      identity: source.identity,
      download_url: catalogSource.source.url,
      page_url: catalogSource.license.evidenceUrl,
      license: catalogSource.license.spdx,
      attribution: catalogSource.license.attribution ?? catalogSource.title,
    });
  }
  for (const query of recipesDocument.queries) {
    const source = sourceByAssetId(recipesDocument, query.sourceAssetId);
    const searchId = `${query.assetId}-search`;
    const relationship = relationshipBySearchId(relationships, searchId);
    const relevant = [...relationship.relevance].sort((left, right) => right.grade - left.grade);
    assets.push({
      id: query.assetId,
      kind: "static_image",
      role: "query",
      filename: query.filename,
      title: query.title,
      identity: source.identity,
      copy_of: source.assetId,
      derivation: {
        type: "overlay_text",
        text: "ASSET TOOLING COMPAT",
        position: "bottom_right",
      },
      expected_top_match: source.assetId,
      expected_top_k: relevant.map((entry) => entry.assetId),
      expected_non_matches: relationship.hardNegatives,
      capability: query.capability,
      asset_tooling_recipe: query.recipeId,
    });
  }

  const searches = [...baseManifest.searches];
  for (const query of recipesDocument.queries) {
    const source = sourceByAssetId(recipesDocument, query.sourceAssetId);
    const searchId = `${query.assetId}-search`;
    const relationship = relationshipBySearchId(relationships, searchId);
    const relevant = [...relationship.relevance].sort((left, right) => right.grade - left.grade);
    searches.push({
      id: searchId,
      query_asset: query.assetId,
      expected_identity: source.identity,
      expected_top_match: source.assetId,
      expected_top_k: relevant.map((entry) => entry.assetId),
      expected_non_matches: relationship.hardNegatives,
      capability: query.capability,
    });
  }

  return {
    ...baseManifest,
    name: `${baseManifest.name}+asset-tooling-image-quality-v1`,
    description: `${baseManifest.description} Includes reproducible asset-tooling image perturbations and hard negatives.`,
    assets,
    searches,
  };
}

async function materialize({ outputDir, assetToolingRoot, contract, catalogDocument, recipesDocument, relationships, baseManifest }) {
  const packageJson = await verifyAssetToolingCheckout(assetToolingRoot, contract);
  const catalogModule = await importAssetToolingExport(assetToolingRoot, packageJson, contract.exports.catalog, contract.revision);
  const acquisitionModule = await importAssetToolingExport(assetToolingRoot, packageJson, contract.exports.catalogAcquisition, contract.revision);
  const storeModule = await importAssetToolingExport(assetToolingRoot, packageJson, contract.exports.store, contract.revision);
  const codecModule = await importAssetToolingExport(assetToolingRoot, packageJson, contract.exports.imageCodecs, contract.revision);
  const perturbationModule = await importAssetToolingExport(assetToolingRoot, packageJson, contract.exports.imagePerturbations, contract.revision);

  const catalog = catalogModule.createAssetCatalog({
    providers: catalogDocument.providers,
    sources: catalogDocument.sources,
  });
  const toolRoot = path.join(outputDir, ".asset-tooling");
  const acquisitionRoot = path.join(toolRoot, "acquired");
  await rm(toolRoot, { recursive: true, force: true });
  await mkdir(acquisitionRoot, { recursive: true });

  const sourceRuntime = new Map();
  const provenanceSources = [];
  for (const source of recipesDocument.sources) {
    const acquisition = await acquisitionModule.acquireAssetCatalogSource({
      catalog,
      sourceId: source.catalogSourceId,
      destinationRoot: acquisitionRoot,
    });
    const sourceBytes = await readFile(acquisition.filePath);
    const imported = await catalogModule.importAssetCatalogSource(toolRoot, catalog, source.catalogSourceId, sourceBytes);
    const decoded = await codecModule.executeImageDecodeOperation(toolRoot, { inputs: { source: imported.asset } });
    const catalogSource = catalog.getSource(source.catalogSourceId);
    const expectedWidth = catalogSource.metadata.expectedWidth;
    const expectedHeight = catalogSource.metadata.expectedHeight;
    if (decoded.observations.width !== expectedWidth || decoded.observations.height !== expectedHeight) {
      throw new Error(
        `catalog source '${source.catalogSourceId}' decoded to ${decoded.observations.width}x${decoded.observations.height}; expected ${expectedWidth}x${expectedHeight}`,
      );
    }
    const target = path.join(outputDir, source.filename);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, sourceBytes);
    sourceRuntime.set(source.assetId, {
      canonical: decoded.outputs.output,
      catalogSource,
      imported: imported.asset,
    });
    provenanceSources.push({
      assetId: source.assetId,
      catalogSourceId: source.catalogSourceId,
      sourceAsset: imported.asset,
      canonicalAsset: decoded.outputs.output,
      decodeObservations: decoded.observations,
    });
  }

  const recipes = new Map(recipesDocument.recipes.map((entry) => [entry.id, entry]));
  const provenanceQueries = [];
  for (const query of recipesDocument.queries) {
    const runtimeSource = sourceRuntime.get(query.sourceAssetId);
    if (!runtimeSource) throw new Error(`query '${query.assetId}' source was not materialized`);
    const recipe = recipes.get(query.recipeId);
    if (!recipe) throw new Error(`query '${query.assetId}' recipe '${query.recipeId}' was not materialized`);
    const execution = await perturbationModule.executeImagePerturbationRecipe(toolRoot, {
      source: runtimeSource.canonical,
      recipe: recipe.recipe,
    });
    const encoded = await codecModule.executeImageEncodePngOperation(toolRoot, {
      parameters: { compressionLevel: 9 },
      inputs: { source: execution.output },
    });
    const outputBytes = await storeModule.resolveAssetObject(toolRoot, encoded.outputs.output);
    const target = path.join(outputDir, query.filename);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, outputBytes);
    provenanceQueries.push({
      assetId: query.assetId,
      sourceAssetId: query.sourceAssetId,
      recipeId: query.recipeId,
      family: recipe.family,
      recipeSha256: execution.recipeSha256,
      sourceCanonicalAsset: execution.source,
      resultCanonicalAsset: execution.output,
      encodedAsset: encoded.outputs.output,
      steps: execution.steps,
      encodeObservations: encoded.observations,
    });
  }

  const provenance = {
    schemaVersion: 1,
    generator: { repository: contract.repository, revision: contract.revision },
    sources: provenanceSources,
    queries: provenanceQueries,
  };
  await writeJson(path.join(outputDir, "asset-tooling-provenance.json"), provenance);

  const attribution = ["# Asset-tooling quality corpus attribution", ""];
  for (const source of catalogDocument.sources) {
    attribution.push(
      `- **${source.title}** — ${source.license.spdx}; ${source.license.attribution ?? "no attribution required"}; ${source.license.evidenceUrl}`,
    );
  }
  attribution.push("");
  await writeFile(path.join(outputDir, "ASSET_TOOLING_ATTRIBUTION.md"), attribution.join("\n"));

  const compatibilityManifest = buildCompatibilityManifest(
    baseManifest,
    catalogDocument,
    recipesDocument,
    relationships,
  );
  const compatibilityRoot = path.join(outputDir, ".asset-tooling-evaluator-root");
  await rm(compatibilityRoot, { recursive: true, force: true });
  await writeJson(
    path.join(compatibilityRoot, "tests/fixtures/quality-corpus/manifest.json"),
    compatibilityManifest,
  );
  await writeJson(path.join(outputDir, "asset-tooling-materialization.json"), {
    schemaVersion: 1,
    generator: { repository: contract.repository, revision: contract.revision },
    compatibilityRoot: ".asset-tooling-evaluator-root",
    sourceCount: recipesDocument.sources.length,
    queryCount: recipesDocument.queries.length,
  });

  return { outputDir, compatibilityRoot, provenance };
}

async function main() {
  const contract = await readJson(path.join(assetFixtureRoot, "contract.json"));
  const catalogDocument = await readJson(path.join(assetFixtureRoot, "catalog.json"));
  const recipesDocument = await readJson(path.join(assetFixtureRoot, "recipes.json"));
  const relationshipsDocument = await readJson(path.join(assetFixtureRoot, "relationships.json"));
  const baseManifest = await readJson(path.join(fixtureRoot, "manifest.json"));
  const relationships = validateFixtureDocuments(
    contract,
    catalogDocument,
    recipesDocument,
    relationshipsDocument,
    baseManifest,
  );

  const args = process.argv.slice(2);
  if (args.includes("--check")) {
    process.stdout.write(
      `asset-tooling quality fixture contract is valid: ${catalogDocument.sources.length} pinned sources, ${recipesDocument.queries.length} generated queries, ${relationships.searches.length} graded searches\n`,
    );
    return;
  }
  const outputIndex = args.indexOf("--output");
  const outputDir = outputIndex >= 0
    ? path.resolve(repoRoot, requireString(args[outputIndex + 1], "--output"))
    : path.resolve(repoRoot, recipesDocument.outputDir);
  const assetToolingRoot = path.resolve(
    process.env.ASSET_TOOLING_ROOT ?? path.join(repoRoot, contract.defaultSibling),
  );
  const result = await materialize({
    outputDir,
    assetToolingRoot,
    contract,
    catalogDocument,
    recipesDocument,
    relationships,
    baseManifest,
  });
  process.stdout.write(
    `asset-tooling quality corpus materialized: ${recipesDocument.sources.length} sources, ${recipesDocument.queries.length} queries; provenance ${path.join(result.outputDir, "asset-tooling-provenance.json")}\n`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
