import { describe, expect, it } from 'vitest';
import { backgroundSize, coverSource, softenCover, TEXTURE_SIZE } from './albumTexture';

describe('background budget', () => {
  it('bounds pixels even on a high-DPI/4K display', () => {
    expect(backgroundSize(3840, 2160)).toEqual({ width: 480, height: 270 });
    expect(backgroundSize(600, 700)).toEqual({ width: 274, height: 320 });
    expect(backgroundSize(0, 0)).toEqual({ width: 1, height: 1 });
    expect(TEXTURE_SIZE).toBeLessThan(64);
  });
  it('uses small trusted-protocol cover requests', () => {
    expect(coverSource('https://p1.music.126.net/cover.jpg')).toContain('param=96y96');
    expect(coverSource('file:///secret')).toBe('');
    expect(coverSource('javascript:alert(1)')).toBe('');
    expect(coverSource()).toBe('');
  });
  it('preblurs small textures without mutating the original', () => {
    const input = new Uint8ClampedArray(8 * 8 * 4);
    input[0] = 255; input[3] = 255;
    const result = softenCover(input, 8, 8);
    expect(input[0]).toBe(255);
    expect(result.length).toBe(input.length);
    expect(result[0]).toBeLessThan(255);
    expect(result[4]).toBeGreaterThan(26);
    expect(result[3]).toBe(255);
    expect(result[result.length - 1]).toBe(255);
  });
});
