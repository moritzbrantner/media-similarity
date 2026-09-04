import type { MediaKind } from "./showcaseRuntime";

const DATABASE_NAME = "media-similarity-showcase";
const DATABASE_VERSION = 1;
const UPLOAD_STORE = "uploads";

export type PersistedUpload = {
  blob: Blob;
  createdAt: number;
  id: string;
  kind: MediaKind;
  label: string;
  mimeType: string;
};

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
  });
}

function openDatabase(): Promise<IDBDatabase> {
  if (typeof indexedDB === "undefined") {
    return Promise.reject(new Error("IndexedDB is unavailable in this browser"));
  }

  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);

    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(UPLOAD_STORE)) {
        database.createObjectStore(UPLOAD_STORE, { keyPath: "id" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Could not open browser media storage"));
  });
}

export function createUploadId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `upload:${crypto.randomUUID()}`;
  }

  return `upload:${Date.now().toString(36)}:${Math.random().toString(36).slice(2)}`;
}

export async function listPersistedUploads(): Promise<PersistedUpload[]> {
  const database = await openDatabase();

  try {
    const transaction = database.transaction(UPLOAD_STORE, "readonly");
    const done = transactionDone(transaction);
    const request = transaction.objectStore(UPLOAD_STORE).getAll();
    const uploads = (await requestResult(request)) as PersistedUpload[];
    await done;
    return uploads.sort((left, right) => left.createdAt - right.createdAt);
  } finally {
    database.close();
  }
}

export async function savePersistedUpload(upload: PersistedUpload): Promise<void> {
  const database = await openDatabase();

  try {
    const transaction = database.transaction(UPLOAD_STORE, "readwrite");
    const done = transactionDone(transaction);
    transaction.objectStore(UPLOAD_STORE).put(upload);
    await done;
  } finally {
    database.close();
  }
}

export async function deletePersistedUpload(id: string): Promise<void> {
  const database = await openDatabase();

  try {
    const transaction = database.transaction(UPLOAD_STORE, "readwrite");
    const done = transactionDone(transaction);
    transaction.objectStore(UPLOAD_STORE).delete(id);
    await done;
  } finally {
    database.close();
  }
}

export async function clearPersistedUploads(): Promise<void> {
  const database = await openDatabase();

  try {
    const transaction = database.transaction(UPLOAD_STORE, "readwrite");
    const done = transactionDone(transaction);
    transaction.objectStore(UPLOAD_STORE).clear();
    await done;
  } finally {
    database.close();
  }
}
