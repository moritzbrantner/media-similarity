const healthEl = document.querySelector("#health");
const statusEl = document.querySelector("#status");
const resultCountEl = document.querySelector("#resultCount");
const resultsEl = document.querySelector("#results");
const previewImage = document.querySelector("#previewImage");
const imageInput = document.querySelector("#imageInput");
const limitInput = document.querySelector("#limitInput");
const indexButton = document.querySelector("#indexButton");
const searchForm = document.querySelector("#searchForm");

async function loadHealth() {
  try {
    const response = await fetch("/api/health");
    const health = await response.json();
    const sources = health.sources?.length ? health.sources.join(", ") : health.source_dir;
    healthEl.textContent = `${health.status.toUpperCase()} | Sources: ${sources} | Collection: ${health.collection}`;
  } catch {
    healthEl.textContent = "Service is not responding";
  }
}

function setStatus(message, tone = "") {
  statusEl.textContent = message;
  statusEl.dataset.tone = tone;
}

function clearResults() {
  resultCountEl.textContent = "";
  resultsEl.innerHTML = "";
}

imageInput.addEventListener("change", () => {
  const file = imageInput.files?.[0];
  if (!file) {
    previewImage.removeAttribute("src");
    return;
  }
  previewImage.src = URL.createObjectURL(file);
});

indexButton.addEventListener("click", async () => {
  indexButton.disabled = true;
  setStatus("Indexing configured sources. The first run can take a while while the model loads.", "");
  try {
    const response = await fetch("/api/index", { method: "POST" });
    const payload = await response.json();
    if (!response.ok) {
      throw new Error(payload.detail || "Indexing failed");
    }
    const errorText = payload.errors?.length ? ` Errors: ${payload.errors.length}` : "";
    setStatus(`Indexed ${payload.indexed} image(s). Failed ${payload.failed}.${errorText}`, payload.failed ? "warn" : "ok");
  } catch (error) {
    setStatus(error.message, "error");
  } finally {
    indexButton.disabled = false;
  }
});

searchForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  clearResults();

  const file = imageInput.files?.[0];
  if (!file) {
    setStatus("Choose an image first.", "error");
    return;
  }

  const formData = new FormData();
  formData.append("file", file);

  const limit = Number(limitInput.value || 12);
  setStatus("Searching...", "");

  try {
    const response = await fetch(`/api/search?limit=${encodeURIComponent(limit)}`, {
      method: "POST",
      body: formData,
    });
    const payload = await response.json();
    if (!response.ok) {
      throw new Error(payload.detail || "Search failed");
    }
    renderResults(payload.results);
    resultCountEl.textContent = `${payload.count} result(s) | Query pHash ${payload.query_phash}`;
    setStatus("Search complete.", "ok");
  } catch (error) {
    setStatus(error.message, "error");
  }
});

function renderResults(results) {
  if (!results.length) {
    resultsEl.innerHTML = `<p class="empty">No indexed images matched this query.</p>`;
    return;
  }

  resultsEl.innerHTML = results
    .map((result) => {
      const image = result.image;
      const duplicateBadge = result.near_duplicate ? `<span class="badge">Near duplicate</span>` : "";
      const score = Number(result.vector_score).toFixed(4);
      return `
        <article class="result-card">
          <div class="thumb-wrap">
            ${image.thumbnail_url ? `<img src="${escapeHtml(image.thumbnail_url)}" alt="" loading="lazy" />` : ""}
          </div>
          <div class="result-body">
            <div class="result-title" title="${escapeHtml(image.relative_path)}">${escapeHtml(image.filename)}</div>
            <div class="path" title="${escapeHtml(image.relative_path)}">${escapeHtml(image.relative_path)}</div>
            <dl>
              <div><dt>CLIP score</dt><dd>${score}</dd></div>
              <div><dt>pHash distance</dt><dd>${result.hash_distance}</dd></div>
              <div><dt>Size</dt><dd>${image.width} x ${image.height}</dd></div>
            </dl>
            ${duplicateBadge}
          </div>
        </article>
      `;
    })
    .join("");
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => {
    const escapes = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#039;",
    };
    return escapes[char];
  });
}

loadHealth();
