export type SimilaritySearchHit = {
  index: number;
  score: number;
};

type WasmSimilarityIndex = {
  add(imageBytes: Uint8Array): number;
  addFeatures(features: Float32Array): number;
  free(): void;
  search(queryImageBytes: Uint8Array, limit: number): ArrayLike<number>;
  searchFeatures(queryFeatures: Float32Array, limit: number): ArrayLike<number>;
};

type WasmBindings = {
  default(input?: unknown): Promise<unknown>;
  SimilarityIndex: new () => WasmSimilarityIndex;
};

export type SimilarityIndexClient = {
  addImage(imageBytes: Uint8Array): number;
  addFeatures(features: Float32Array): number;
  free(): void;
  searchImage(queryImageBytes: Uint8Array, limit: number): SimilaritySearchHit[];
  searchFeatures(queryFeatures: Float32Array, limit: number): SimilaritySearchHit[];
};

let bindingsPromise: Promise<WasmBindings> | null = null;

function decodeHits(flattened: ArrayLike<number>): SimilaritySearchHit[] {
  const values = Array.from(flattened);
  const hits: SimilaritySearchHit[] = [];

  for (let offset = 0; offset + 1 < values.length; offset += 2) {
    hits.push({
      index: Math.trunc(values[offset]),
      score: values[offset + 1],
    });
  }

  return hits;
}

async function loadBindings(): Promise<WasmBindings> {
  if (!bindingsPromise) {
    const moduleUrl = new URL("wasm/media_similarity_wasm.js", document.baseURI).href;
    bindingsPromise = import(/* @vite-ignore */ moduleUrl).then(async (module) => {
      const bindings = module as WasmBindings;
      await bindings.default();
      return bindings;
    });
  }

  return bindingsPromise;
}

export async function createSimilarityIndex(): Promise<SimilarityIndexClient> {
  const bindings = await loadBindings();
  const index = new bindings.SimilarityIndex();

  return {
    addImage: (imageBytes) => index.add(imageBytes),
    addFeatures: (features) => index.addFeatures(features),
    free: () => index.free(),
    searchImage: (queryImageBytes, limit) => decodeHits(index.search(queryImageBytes, limit)),
    searchFeatures: (queryFeatures, limit) =>
      decodeHits(index.searchFeatures(queryFeatures, limit)),
  };
}
