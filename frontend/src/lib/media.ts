const AUDIO_EXTENSIONS = [".mp3", ".wav", ".flac", ".m4a", ".aac", ".ogg", ".opus", ".wma"];
const PDF_EXTENSIONS = [".pdf"];

export function isAudioFile(file: File) {
  if (file.type.startsWith("audio/")) {
    return true;
  }
  const name = file.name.toLocaleLowerCase();
  return AUDIO_EXTENSIONS.some((extension) => name.endsWith(extension));
}

export function isPdfFile(file: File) {
  if (file.type === "application/pdf") {
    return true;
  }
  const name = file.name.toLocaleLowerCase();
  return PDF_EXTENSIONS.some((extension) => name.endsWith(extension));
}
