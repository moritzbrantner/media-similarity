export async function createQueryPreview(file: File) {
  if (file.type === "image/gif" || file.name.toLowerCase().endsWith(".gif")) {
    return null;
  }

  try {
    const image = await createImageBitmap(file);
    const maxSize = 640;
    const scale = Math.min(1, maxSize / Math.max(image.width, image.height));
    const width = Math.max(1, Math.round(image.width * scale));
    const height = Math.max(1, Math.round(image.height * scale));
    const canvas = document.createElement("canvas");

    canvas.width = width;
    canvas.height = height;
    canvas.getContext("2d")?.drawImage(image, 0, 0, width, height);
    image.close();

    return canvas.toDataURL("image/jpeg", 0.82);
  } catch {
    return null;
  }
}
