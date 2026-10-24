import { describe, expect, it } from 'vitest';
import { activeLyricIndex, displayPosition, normalizeLyrics } from './lyrics';

describe('line lyrics', () => {
  it('sorts without mutating, preserves translations, rejects invalid times and handles empty text', () => {
    const input = [{ time_ms: 2000.9, text: '', translation: '' }, { time_ms: -1, text: 'invalid', translation: '' }, { time_ms: 0, text: ' intro ', translation: '引子' }];
    const result = normalizeLyrics(input);
    expect(result).toEqual([{ time_ms: 0, text: 'intro', translation: '引子' }, { time_ms: 2000, text: '♪', translation: '' }]);
    expect(input[0].time_ms).toBe(2000.9);
    expect(input[2].text).toBe(' intro ');
  });
  it('finds the current line before, on, and after timestamps, including backward seeks', () => {
    const lines = [1000, 5000, 5000, 10000].map(time_ms => ({ time_ms, text: 'line', translation: '' }));
    expect(activeLyricIndex([], 2000)).toBe(-1);
    expect(activeLyricIndex(lines, 0)).toBe(-1);
    expect(activeLyricIndex(lines, 1000)).toBe(0);
    expect(activeLyricIndex(lines, 5000)).toBe(2);
    expect(activeLyricIndex(lines, 9999)).toBe(2);
    expect(activeLyricIndex(lines, 20000)).toBe(3);
    expect(activeLyricIndex(lines, 3000)).toBe(0);
  });
});

describe('display clock', () => {
  it('interpolates only playing state and clamps at duration', () => {
    expect(displayPosition(12000, 100, 600, true, 180000)).toBe(12500);
    expect(displayPosition(12000, 100, 600, false, 180000)).toBe(12000);
    expect(displayPosition(179900, 100, 600, true, 180000)).toBe(180000);
  });
  it('reanchors on a seek or track change and handles unknown duration', () => {
    expect(displayPosition(1000, 700, 800, true, 0)).toBe(1100);
    expect(displayPosition(0, 800, 800, true, 180000)).toBe(0);
    expect(displayPosition(0, 800, 700, true, 180000)).toBe(0);
  });
});
