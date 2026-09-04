export type SimilaritySearchHit = {
  index: number;
  score: number;
};

type WasmSimilarityIndex = {
  add(imageBytes: Uint8Array): number;
  free(): void;
  search(queryImageBytes: Uint8Array, limit: number): ArrayLike<number>;
};

type WasmBindings = {
  default(input?: unknown): Promise<unknown>;
  SimilarityIndex: new () => WasmSimilarityIndex;
};

export type SimilarityIndexClient = {
  add(imageBytes: Uint8Array): number;
  free(): void;
  search(queryImageBytes: Uint8Array, limit: number): SimilaritySearchHit[];
};

let bindingsPromise: Promise<WasmBindings> | null = null;

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
    add: (imageBytes) => index.add(imageBytes),
    free: () => index.free(),
    search: (queryImageBytes, limit) => {
      const flattened = Array.from(index.search(queryImageBytes, limit));
      const hits: SimilaritySearchHit[] = [];

      for (let offset = 0; offset + 1 < flattened.length; offset += 2) {
        hits.push({
          index: Math.trunc(flattened[offset]),
          score: flattened[offset + 1],
        });
      }

      return hits;
    },
  };
}
