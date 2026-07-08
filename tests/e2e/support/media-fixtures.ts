export * from "../../../frontend/src/testing/media-fixtures";
import { pngPixelBase64 } from "../../../frontend/src/testing/media-fixtures";

export const pngPixel = Buffer.from(pngPixelBase64, "base64");

export const imageUpload = {
  buffer: pngPixel,
  mimeType: "image/png",
  name: "query.png",
};

export const gifUpload = {
  buffer: Buffer.from("GIF89a"),
  mimeType: "image/gif",
  name: "query.gif",
};

export const videoUpload = {
  buffer: Buffer.from("mock video"),
  mimeType: "video/mp4",
  name: "query.mp4",
};

export const audioUpload = {
  buffer: Buffer.from("mock audio"),
  mimeType: "audio/mpeg",
  name: "query.mp3",
};

export const pdfUpload = {
  buffer: Buffer.from("%PDF-1.4\n"),
  mimeType: "application/pdf",
  name: "query.pdf",
};
