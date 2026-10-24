/** Small, bounded image work; no full-window CSS filters or third-party code. */
export const TEXTURE_SIZE = 48;
export const BACKGROUND_FPS = 24;

export function backgroundSize(width: number, height: number) {
  // The backdrop is deliberately soft: DPR must not multiply its pixel budget.
  const scale = Math.min(.5, 480 / Math.max(1, width), 320 / Math.max(1, height));
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

/** Separable edge-clamped box blur, applied only when a cover changes. */
export function softenCover(source: Uint8ClampedArray, width: number, height: number): Uint8ClampedArray {
  let pixels = new Uint8ClampedArray(source);
  const radius = 5, divisor = radius * 2 + 1;
  for (let pass = 0; pass < 3; pass++) {
    for (const horizontal of [true, false]) {
      const output = new Uint8ClampedArray(pixels.length);
      for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
        const dest = (y * width + x) * 4;
        for (let channel = 0; channel < 4; channel++) {
          let sum = 0;
          for (let offset = -radius; offset <= radius; offset++) {
            const sx = horizontal ? Math.max(0, Math.min(width - 1, x + offset)) : x;
            const sy = horizontal ? y : Math.max(0, Math.min(height - 1, y + offset));
            sum += pixels[(sy * width + sx) * 4 + channel];
          }
          output[dest + channel] = sum / divisor;
        }
      }
      pixels = output;
    }
  }
  // Keep broad cover colours but lower contrast for text readability.
  for (let i = 0; i < pixels.length; i += 4) {
    for (let c = 0; c < 3; c++) pixels[i + c] = pixels[i + c] * .72 + 26;
    pixels[i + 3] = 255;
  }
  return pixels;
}

export function coverSource(cover?: string): string {
  try {
    const url = new URL(cover ?? '');
    if (!['http:', 'https:', 'data:'].includes(url.protocol)) return '';
    if (url.hostname.endsWith('.music.126.net')) url.searchParams.set('param', '96y96');
    return url.toString();
  } catch { return ''; }
}

// Bounded cache survives closing/reopening the sheet; canvas textures are tiny.
const textures = new Map<string, HTMLCanvasElement>();
export function cachedTexture(source: string) { return textures.get(source); }
export function createCoverTexture(image: HTMLImageElement, source: string): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = TEXTURE_SIZE;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) throw new Error('Canvas unavailable');
  const side = Math.min(image.naturalWidth, image.naturalHeight);
  if (!side) throw new Error('Invalid cover dimensions');
  context.drawImage(image, (image.naturalWidth - side) / 2, (image.naturalHeight - side) / 2, side, side, 0, 0, TEXTURE_SIZE, TEXTURE_SIZE);
  const data = context.getImageData(0, 0, TEXTURE_SIZE, TEXTURE_SIZE);
  data.data.set(softenCover(data.data, TEXTURE_SIZE, TEXTURE_SIZE));
  context.putImageData(data, 0, 0);
  textures.delete(source);
  textures.set(source, canvas);
  if (textures.size > 6) textures.delete(textures.keys().next().value!);
  return canvas;
}
