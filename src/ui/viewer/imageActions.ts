import { sentImageData } from "../../core/client";
import { sentImage } from "../../core/sentImage";

export async function copyImage(project: string, path: string, loaded: HTMLImageElement) {
  if (typeof ClipboardItem === "undefined" || !navigator.clipboard?.write) {
    throw new Error("Image clipboard unavailable");
  }
  // Keep the clipboard request in the menu's user activation while the PNG is prepared.
  await navigator.clipboard.write([new ClipboardItem({ "image/png": clipboardPng(project, path, loaded) })]);
}

async function clipboardPng(project: string, path: string, image: HTMLImageElement): Promise<Blob> {
  const delivery = sentImage(path);
  if (delivery !== null) {
    // A delivery's custom-protocol URL would taint the clipboard canvas.
    image = new Image();
    image.src = await sentImageData(project, delivery.id);
    await image.decode();
  }
  const canvas = document.createElement("canvas");
  canvas.width = image.naturalWidth;
  canvas.height = image.naturalHeight;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Image canvas unavailable");
  ctx.drawImage(image, 0, 0);
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error("Could not encode the clipboard image"));
    }, "image/png");
  });
}
