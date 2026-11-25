import { convertFileSrc } from "@tauri-apps/api/core";
import { api } from "./api";

let dir: string | null = null;

/** Load the preview cache folder once at startup. */
export async function initPreviews() {
  try {
    dir = (await api.previewsDir()) || null;
  } catch {
    dir = null;
  }
}

/** URL for a cached preview image or favicon, served via Tauri's asset protocol. */
export function previewSrc(name: string | null | undefined): string | null {
  if (!name || !dir || !/^[0-9a-f]{64}\.(png|jpg|gif|webp|ico|avif)$/.test(name)) return null;
  return convertFileSrc(`${dir}\${name}`);
}
