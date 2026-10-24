import { describe, expect, it } from 'vitest';
import { advanceLyricSpring, lyricStagger, shouldAnimateLyricChange } from './lyricMotion';

describe('lyric spring', () => {
  it('starts continuously and lands with only a small overshoot', () => {
    let state = { position: 140, velocity: 0 };
    expect(advanceLyricSpring(state, 0)).toEqual(state);
    let minimum = state.position;
    for (let i = 0; i < 180; i++) {
      state = advanceLyricSpring(state, 1000 / 60);
      minimum = Math.min(minimum, state.position);
      expect(Number.isFinite(state.velocity)).toBe(true);
    }
    expect(minimum).toBeLessThan(0);
    expect(minimum).toBeGreaterThan(-2);
    expect(state).toEqual({ position: 0, velocity: 0 });
  });
  it('is stable across refresh rates and long dropped frames', () => {
    const initial = { position: 200, velocity: -70 };
    const whole = advanceLyricSpring(initial, 300);
    let segmented = initial;
    for (let i = 0; i < 36; i++) segmented = advanceLyricSpring(segmented, 300 / 36);
    expect(segmented.position).toBeCloseTo(whole.position, 8);
    expect(segmented.velocity).toBeCloseTo(whole.velocity, 8);
    expect(advanceLyricSpring(initial, 10000)).toEqual({ position: 0, velocity: 0 });
  });
  it('can retarget a fast phrase without discarding its current velocity', () => {
    const moving = advanceLyricSpring({ position: 100, velocity: 0 }, 150);
    const next = advanceLyricSpring({ position: moving.position + 100, velocity: moving.velocity }, 16);
    expect(next.position).toBeLessThan(moving.position + 100);
    expect(next.velocity).toBeLessThan(0);
  });
  it('lets the active line lead with a bounded incoming cascade', () => {
    expect([0, 1, 2, 3, 4, 5, 100].map(i => lyricStagger(i, 1))).toEqual([0, 0, 38, 76, 114, 140, 140]);
  });
  it('animates normal boundaries but not scrubbing, startup or backward jumps', () => {
    expect(shouldAnimateLyricChange(1, 2, 16)).toBe(true);
    expect(shouldAnimateLyricChange(-1, 0, 16)).toBe(false);
    expect(shouldAnimateLyricChange(2, 1, -16)).toBe(false);
    expect(shouldAnimateLyricChange(1, 4, 16)).toBe(false);
    expect(shouldAnimateLyricChange(1, 2, 2000)).toBe(false);
  });
});
