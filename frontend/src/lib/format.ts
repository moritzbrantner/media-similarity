export function mediaKindLabel(kind: string) {
  return kind.replaceAll("_", " ");
}

export function formatFileSize(sizeBytes: number) {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }
  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }
  return `${(sizeBytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatModifiedAt(modifiedAt: number) {
  if (!Number.isFinite(modifiedAt) || modifiedAt <= 0) {
    return "n/a";
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(new Date(modifiedAt * 1000));
}
