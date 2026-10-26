import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  // This fixture validates UI behavior only. Native playback has its own real
  // GStreamer integration test; window lifecycle runs against the Tauri binary.
  await page.addInitScript(() => {
    const callbacks = new Map<number, (data: unknown) => void>();
    const listeners = new Map<string, number>();
    let sequence = 0;
    const tracks = Array.from({ length: 10000 }, (_, i) => ({ id: i + 1, name: `测试歌曲 ${i + 1}`, artist: '测试歌手', album: '测试专辑', album_id: 1, cover: '', duration_ms: 180000, playable: true, source: null }));
    let state = { revision: 1, queue_revision: 1, queue: tracks.slice(0, 3), current_id: 1, playback: 'paused', position_ms: 12000, duration_ms: 180000, volume: .8, repeat: 'off', shuffle: false, continuous: { kind: 'none' }, error: null };
    const settings = { theme: 'light', quality: 'standard', volume: .8, mute_start: false, proxy: '', cache_days: 0, try_unavailable: false, desktop_lyrics: false, reduced_motion: true };
    let loggedIn = true; let subscribed = false;
    let maximized = false;
    const windowCommands: string[] = [];
    const likes = new Set<number>();
    const internals = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } },
      transformCallback(callback: (data: unknown) => void) { callbacks.set(++sequence, callback); return sequence; },
      unregisterCallback(id: number) { callbacks.delete(id); },
      async invoke(cmd: string, args: any = {}) {
        if (cmd.startsWith('plugin:window|')) {
          windowCommands.push(cmd);
          if (cmd === 'plugin:window|is_maximized') return maximized;
          if (cmd === 'plugin:window|toggle_maximize') maximized = !maximized;
          if (cmd === 'plugin:window|close') {
            callbacks.get(listeners.get('tauri://close-requested') ?? 0)?.({ event: 'tauri://close-requested', payload: null });
          }
          return;
        }
        if (cmd === 'plugin:event|listen') { listeners.set(args.event, args.handler); return args.handler; }
        if (cmd === 'plugin:event|unlisten') return;
        if (cmd === 'player_snapshot') return structuredClone(state);
        if (cmd === 'settings') return settings;
        if (cmd === 'save_settings') return Object.assign(settings, args.settings);
        if (cmd === 'ui_state') {
          if (args.value) sessionStorage.setItem('test-ui', JSON.stringify(args.value));
          return JSON.parse(sessionStorage.getItem('test-ui') ?? '{}');
        }
        if (cmd === 'account') {
          if (args.operation === 'status') return { uid: loggedIn ? 1 : 0, nickname: '音乐旅人', avatar_url: '', code: 200 };
          if (args.operation === 'likes') return [...likes];
          if (args.operation === 'logout') loggedIn = false;
          return {};
        }
        if (cmd === 'favorite') {
          if (args.kind === 'song') { args.liked ? likes.add(args.id) : likes.delete(args.id); }
          else subscribed = args.liked;
          return;
        }
        if (cmd === 'cache') return 0;
        if (cmd === 'lyrics') return [{ time_ms: 0, text: '第一句歌词', translation: '' }, { time_ms: 10000, text: '此刻正唱到这里', translation: 'Here and now' }];
        if (cmd === 'browse') {
          if (args.query.text === 'failure') throw new Error('测试网络错误');
          if (['playlist', 'favorites', 'search_song'].includes(args.query.kind)) return { title: '测试歌单', subtitle: '用于验证长列表与播放器交互', cover: '', tracks, cards: [], more: false, subscribed };
          return { title: '', subtitle: '', cover: '', tracks: [], cards: Array.from({ length: 12 }, (_, i) => ({ id: i + 1, name: ['午后，和音乐一起慢下来', '城市夜行 · 慢拍电子', '那些陪我们长大的旋律'][i % 3], subtitle: '精选音乐', cover: '', kind: args.query.kind === 'albums' ? 'album' : 'playlist' })), more: false };
        }
        if (cmd === 'player_command') {
          const command = args.command;
          if (command.type === 'toggle') state.playback = state.playback === 'playing' ? 'paused' : 'playing';
          if (command.type === 'replace') { state.queue = command.tracks; state.current_id = command.tracks[command.index].id; state.playback = 'playing'; state.queue_revision++; }
          if (command.type === 'select') { state.current_id = command.id; state.playback = 'playing'; }
          if (command.type === 'seek') state.position_ms = command.position_ms;
          if (command.type === 'volume') state.volume = command.value;
          if (command.type === 'mode') { state.repeat = command.repeat; state.shuffle = command.shuffle; }
          state.revision++;
          const callback = callbacks.get(listeners.get('player-state') ?? 0);
          callback?.({ event: 'player-state', payload: { ...state } });
        }
      },
    };
    Object.assign(window, { isTauri: true, __windowCommands: windowCommands, __TAURI_INTERNALS__: internals, __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} } });
  });
});

test('collection toggles and settings drafts survive playback updates', async ({ page }) => {
  await page.goto('/#/playlist/1');
  const favorite = page.locator('.detail-actions button[aria-pressed]');
  await expect(favorite).toHaveAttribute('aria-label', '收藏');
  await favorite.click();
  await expect(favorite).toHaveAttribute('aria-label', '取消收藏');
  await expect(favorite).toHaveAttribute('aria-pressed', 'true');
  await favorite.click();
  await expect(favorite).toHaveAttribute('aria-pressed', 'false');
  await page.getByRole('link', { name: '设置', exact: true }).click();
  await page.getByLabel('主题').selectOption('dark');
  await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('player_command', { command: { type: 'volume', value: 0.4 } }));
  await expect(page.getByLabel('主题')).toHaveValue('dark');
  await page.getByRole('button', { name: '保存设置' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: '退出登录', exact: true }).click();
  await expect(page.locator('.settings-section').first()).toContainText('尚未登录');
});

test('navigation, bottom player and queue share core state', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '发现音乐.' })).toBeVisible();
  await page.getByRole('button', { name: '播放', exact: true }).click();
  await expect(page.getByRole('button', { name: '暂停', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '播放队列', exact: true }).click();
  await expect(page.getByRole('complementary', { name: '播放队列' })).toBeVisible();
  await page.getByRole('button', { name: '测试歌曲 2 测试歌手', exact: true }).click();
  await expect(page.locator('.track-info strong')).toHaveText('测试歌曲 2');
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  await expect(page.locator('.lyric-lines button.active')).toContainText('此刻正唱到这里');
  await page.keyboard.press('Escape'); // close the queue first
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: '正在播放' })).toHaveCount(0);
});

test('expanded player is modal and returns to the underlying route and trigger', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.connection .online')).toBeVisible();
  for (const [name, path] of [['发现音乐', '/discover'], ['排行榜', '/charts'], ['喜欢的音乐', '/favorites'], ['设置', '/settings']]) {
    await page.getByRole('button', { name: '歌词', exact: true }).click();
    await expect(page.getByRole('region', { name: '歌词', exact: true })).toBeVisible();
    await expect(page.getByRole('link', { name: '发现音乐', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: '关闭歌词', exact: true }).click();
    await expect(page.getByRole('button', { name: '歌词', exact: true })).toBeFocused();
    await page.locator('.sidebar').getByRole('link', { name, exact: true }).click();
    await expect(page.getByRole('dialog', { name: '正在播放' })).toHaveCount(0);
    await expect(page).toHaveURL(new RegExp(`#${path}$`));
  }
  await expect(page.locator('.settings-page')).toBeVisible();
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  await page.keyboard.press('Escape');
  await page.locator('.brand').click();
  await expect(page.getByRole('region', { name: '歌词', exact: true })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: '发现音乐.' })).toBeVisible();
});

test('quality shortcuts save preferences and playback motion respects reduced motion', async ({ page }) => {
  await page.goto('/');
  const bar = page.getByRole('contentinfo', { name: '播放器' });
  await bar.getByRole('button', { name: '切换首选音质' }).click();
  await bar.getByRole('menuitemradio', { name: '无损', exact: true }).click();
  await expect(bar.getByRole('button', { name: '切换首选音质' })).toHaveText('无损');
  await expect(page.locator('.toast')).toContainText('下次加载歌曲时生效');
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const lyrics = page.getByRole('dialog', { name: '正在播放', exact: true });
  await expect(lyrics.getByRole('button', { name: '切换首选音质' })).toHaveText('无损');
  await lyrics.getByRole('button', { name: '切换首选音质' }).click();
  await lyrics.getByRole('menuitemradio', { name: 'Hi-Res', exact: true }).click();
  await expect(lyrics.getByRole('button', { name: '切换首选音质' })).toHaveText('Hi-Res');
  await lyrics.getByRole('button', { name: '播放', exact: true }).click();
  await expect(lyrics).toHaveClass(/is-playing/);
  const cover = lyrics.locator('.album-sleeve .cover');
  await expect(cover).toHaveCSS('animation-name', 'none');
  await expect(cover).toHaveCSS('border-radius', '14px');
  await expect(lyrics.locator('.lyric-lines')).toBeVisible();
  await lyrics.getByRole('button', { name: '暂停', exact: true }).click();
  await expect(lyrics).not.toHaveClass(/is-playing/);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(cover).toHaveCSS('animation-name', 'none');
  await lyrics.getByRole('button', { name: '关闭歌词' }).click();
  await expect(bar.getByRole('button', { name: '切换首选音质' })).toHaveText('Hi-Res');
  await page.getByRole('link', { name: '设置', exact: true }).click();
  await expect(page.locator('.settings-page').getByRole('combobox', { name: /音质/ })).toHaveValue('hires');
});

test('large song list is virtualized and can play scrolled songs', async ({ page }) => {
  await page.goto('/#/playlist/1');
  await expect(page.getByRole('heading', { name: '测试歌单' })).toBeVisible();
  expect(await page.locator('.track-row').count()).toBeLessThan(50);
  await page.locator('.content-scroll').evaluate(el => { el.scrollTop = 320000; });
  await expect.poll(() => page.locator('.track-row').first().getAttribute('aria-posinset')).not.toBe('1');
  expect(await page.locator('.track-row').count()).toBeLessThan(50);
  await page.locator('.track-row .song-title').first().click();
  await expect(page.getByRole('button', { name: '暂停', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '播放队列', exact: true }).click();
  expect(await page.locator('.queue-item').count()).toBeLessThan(40);
});

test('keyboard search, error recovery and narrow dark layout', async ({ page }) => {
  await page.goto('/');
  await page.keyboard.press('Control+f');
  await expect(page.getByRole('textbox', { name: '搜索音乐' })).toBeFocused();
  await page.getByRole('textbox', { name: '搜索音乐' }).fill('failure');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('alert')).toContainText('测试网络错误');
  await page.getByRole('link', { name: '设置', exact: true }).click();
  await page.getByLabel('主题').selectOption('dark');
  await page.getByRole('button', { name: '保存设置' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.setViewportSize({ width: 780, height: 600 });
  const hasOverflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  expect(hasOverflow).toBe(false);
  await expect(page.getByRole('contentinfo', { name: '播放器' })).toBeVisible();
});

test('immersive lyrics support seek, queue, keyboard and system motion preferences', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/#/settings');
  await page.getByRole('checkbox', { name: /减少动态效果/ }).uncheck();
  await page.getByRole('button', { name: '保存设置' }).click();
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: '正在播放' });
  await expect(sheet.locator('.lyric-lines')).toBeVisible();
  await expect(sheet.locator('.lyric-lines button.active')).toContainText('Here and now');
  await sheet.getByRole('button', { name: '播放', exact: true }).click();
  await expect(sheet.getByRole('button', { name: '暂停', exact: true })).toBeVisible();
  // No cover in this fixture: keep the static gradient without a render loop.
  await expect(sheet.locator('.album-background')).toHaveAttribute('data-motion', 'static');
  await sheet.locator('.lyric-lines').hover();
  await page.mouse.wheel(0, 150);
  await expect(sheet.getByRole('button', { name: '返回当前歌词' })).toBeVisible();
  await sheet.getByRole('button', { name: '返回当前歌词' }).click();
  await expect(sheet.getByRole('button', { name: '返回当前歌词' })).toHaveCount(0);
  await sheet.getByRole('button', { name: '暂停', exact: true }).click();
  await expect(sheet.locator('.album-background')).toHaveAttribute('data-motion', 'static');
  await sheet.getByRole('button', { name: '第一句歌词', exact: true }).click();
  await expect(sheet.getByRole('slider', { name: '播放进度' })).toHaveValue('0');
  await sheet.getByRole('button', { name: '播放队列', exact: true }).click();
  await expect(sheet.getByRole('button', { name: '关闭队列' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(sheet).toBeVisible();
  await expect(sheet.getByRole('button', { name: '播放队列', exact: true })).toBeFocused();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(sheet.locator('.lyrics-content')).toHaveClass(/simple-lyrics/);
  await expect(sheet.locator('.album-background')).toHaveAttribute('data-motion', 'static');
  await expect(sheet.locator('.lyric-lines')).toBeVisible();
  await sheet.getByRole('button', { name: '此刻正唱到这里 Here and now' }).click();
  await expect(sheet.getByRole('slider', { name: '播放进度' })).toHaveValue('10000');
  await page.setViewportSize({ width: 600, height: 700 });
  await expect(sheet.locator('.lyrics-pane')).toBeVisible();
  expect(await sheet.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.keyboard.press('Control+f');
  await expect(sheet).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: '搜索音乐' })).toBeFocused();
  expect(errors).toEqual([]);
});

test('lyric request failures, empty data and repeated opening do not break controls', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.connection .online')).toBeVisible();
  await page.evaluate(() => {
    const ipc = (window as any).__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    ipc.invoke = (cmd: string, args: unknown) => cmd === 'lyrics' ? Promise.reject(new Error('offline')) : invoke(cmd, args);
  });
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('暂时无法获取歌词');
  await page.getByRole('button', { name: '播放', exact: true }).click();
  await expect(page.getByRole('button', { name: '暂停', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.evaluate(() => {
    const ipc = (window as any).__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    ipc.invoke = (cmd: string, args: unknown) => cmd === 'lyrics' ? Promise.resolve([]) : invoke(cmd, args);
  });
  for (let i = 0; i < 3; i++) {
    await page.getByRole('button', { name: '歌词', exact: true }).click();
    await expect(page.getByRole('dialog')).toContainText('此歌曲暂无歌词');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('button', { name: '歌词', exact: true })).toBeFocused();
  }
});

test('expanded controls share commands and a stale lyric request cannot replace the current track', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.connection .online')).toBeVisible();
  await page.evaluate(() => {
    const ipc = (window as any).__TAURI_INTERNALS__;
    const original = ipc.invoke;
    ipc.invoke = (cmd: string, args: any) => {
      if (cmd === 'lyrics' && args.id === 1) return new Promise(resolve => { (window as any).finishOldLyrics = () => resolve([{ time_ms: 0, text: '过时的歌词', translation: '' }]); });
      if (cmd === 'lyrics') return Promise.resolve([{ time_ms: 0, text: '第二首的歌词', translation: '' }]);
      return original(cmd, args);
    };
  });
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: '正在播放' });
  await expect(sheet).toContainText('正在加载歌词');
  await sheet.getByRole('button', { name: '播放队列', exact: true }).click();
  await sheet.getByRole('button', { name: '测试歌曲 2 测试歌手', exact: true }).click();
  await expect(sheet.locator('.lyric-lines')).toContainText('第二首的歌词');
  await page.evaluate(() => (window as any).finishOldLyrics());
  await expect(sheet).not.toContainText('过时的歌词');
  await page.keyboard.press('Escape');
  await sheet.getByRole('button', { name: '随机播放' }).click();
  await expect(sheet.getByRole('button', { name: '随机播放' })).toHaveAttribute('aria-pressed', 'true');
  await sheet.getByRole('button', { name: '循环模式：off' }).click();
  await expect(sheet.getByRole('button', { name: '循环模式：all' })).toBeVisible();
  await sheet.getByRole('button', { name: '喜欢当前歌曲' }).click();
  await expect(sheet.getByRole('button', { name: '喜欢当前歌曲' })).toHaveAttribute('aria-pressed', 'true');
  await sheet.getByRole('button', { name: '静音', exact: true }).click();
  await expect(sheet.getByRole('slider', { name: '音量' })).toHaveValue('0');
  await sheet.getByRole('button', { name: '取消静音' }).click();
  await expect(sheet.getByRole('slider', { name: '音量' })).toHaveValue('0.8');
  await sheet.getByRole('button', { name: '关闭歌词' }).click();
  const bar = page.getByRole('contentinfo', { name: '播放器' });
  await expect(bar.getByRole('button', { name: '随机播放' })).toHaveAttribute('aria-pressed', 'true');
  await expect(bar.getByRole('button', { name: '循环模式：all' })).toBeVisible();
});

test('saved expanded player restores after route restoration', async ({ page }) => {
  await page.addInitScript(() => sessionStorage.setItem('test-ui', JSON.stringify({ route: '/charts', lyrics: true, queue: false })));
  await page.goto('/');
  await expect(page).toHaveURL(/#\/charts$/);
  await expect(page.getByRole('dialog', { name: '正在播放' })).toBeVisible();
  await page.getByRole('button', { name: '关闭歌词' }).click();
  await expect(page.getByRole('heading', { name: /排行榜/ })).toBeVisible();
});

test('natural lyric boundaries cascade independently and settle without residual transforms', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/#/settings');
  await page.getByRole('checkbox', { name: /减少动态效果/ }).uncheck();
  await page.getByRole('button', { name: '保存设置' }).click();
  await page.evaluate(() => {
    const ipc = (window as any).__TAURI_INTERNALS__;
    const original = ipc.invoke;
    ipc.invoke = (cmd: string, args: any) => cmd === 'lyrics' ? Promise.resolve(Array.from({ length: 14 }, (_, i) => ({
      time_ms: i === 3 ? 20100 : i === 4 ? 20200 : i * 10000, text: `第${i + 1}句：风穿过了安静的街道`, translation: i % 2 ? 'A longer translated line that can wrap on smaller windows' : '',
    }))) : original(cmd, args);
  });
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: '正在播放' });
  const content = sheet.locator('.lyrics-content');
  await expect(sheet.locator('.lyric-lines button')).toHaveCount(14);
  await expect(sheet).toHaveAttribute('data-phase', 'open');
  async function boundary() {
    return page.evaluate(async () => {
      const ipc = (window as any).__TAURI_INTERNALS__;
      await ipc.invoke('player_command', { command: { type: 'seek', position_ms: 19990 } });
      await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
      await ipc.invoke('player_command', { command: { type: 'seek', position_ms: 20010 } });
      await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      const el = document.querySelector('.lyric-lines')!;
      return [2, 3, 4].map(i => Number.parseFloat((el.querySelector(`[data-line="${i}"]`) as HTMLElement).style.getPropertyValue('--lyric-y') || '0'));
    });
  }
  // Sample in the same renderer task, not after a round-trip assertion which
  // can reach the spring's overshoot/settling phase on a busy host.
  const offsets = await boundary();
  await expect(content).toHaveClass(/lyrics-moving/);
  expect(offsets[0]).toBeLessThan(offsets[1]);
  expect(offsets[1]).toBeLessThanOrEqual(offsets[2]);
  await page.evaluate(async () => {
    const ipc = (window as any).__TAURI_INTERNALS__;
    for (const position_ms of [20110, 20210]) {
      await ipc.invoke('player_command', { command: { type: 'seek', position_ms } });
      await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    }
  });
  await expect(sheet.locator('.lyric-lines button.active')).toHaveAttribute('data-line', '4');
  await expect(content).toHaveClass(/lyrics-moving/);
  await expect(content).not.toHaveClass(/lyrics-moving/);
  await expect(sheet.locator('[style*="--lyric-y"]')).toHaveCount(0);
  await boundary();
  await expect(content).toHaveClass(/lyrics-moving/);
  // Resize while the cascade is in flight: offsets must be discarded and the
  // active variable-height row remeasured at the same viewport anchor.
  await page.setViewportSize({ width: 780, height: 600 });
  await expect(content).not.toHaveClass(/lyrics-moving/);
  await expect(sheet.locator('[style*="--lyric-y"]')).toHaveCount(0);
  const anchor = await sheet.locator('.lyric-lines').evaluate(el => {
    const active = el.querySelector('.active')!.getBoundingClientRect();
    return (active.top + active.height / 2 - el.getBoundingClientRect().top) / el.clientHeight;
  });
  expect(anchor).toBeCloseTo(.44, 2);

  await boundary();
  await expect(content).toHaveClass(/lyrics-moving/);
  await sheet.locator('.lyric-lines').hover();
  await page.mouse.wheel(0, 50);
  await expect(content).toHaveClass(/browsing-lyrics/);
  await expect(content).not.toHaveClass(/lyrics-moving/);
  await expect(sheet.locator('[style*="--lyric-y"]')).toHaveCount(0);
  await sheet.getByRole('button', { name: '返回当前歌词' }).click();
  await boundary();
  await expect(content).toHaveClass(/lyrics-moving/);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(content).not.toHaveClass(/lyrics-moving/);
  await expect(sheet.locator('[style*="--lyric-y"]')).toHaveCount(0);
  await sheet.locator('.lyric-lines').focus();
  await page.keyboard.press('ArrowUp');
  await expect(sheet.locator('.lyric-lines button.active')).toHaveAttribute('data-line', '1');
  // A large seek skips the cascade, including when motion is re-enabled.
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('player_command', { command: { type: 'seek', position_ms: 100000 } }));
  await expect(sheet.locator('.lyric-lines button.active')).toHaveAttribute('data-line', '10');
  await expect(content).not.toHaveClass(/lyrics-moving/);
  await expect(sheet.locator('[style*="--lyric-y"]')).toHaveCount(0);
});

test('player sheet keeps modal focus through entry, exit and a reversed close', async ({ page }) => {
  await page.addInitScript(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args: Parameters<Element['animate']>) {
      const result = animate.apply(this, args);
      // Hold only the sheet transition for deterministic lifecycle assertions.
      if (this.classList.contains('lyrics-view')) result.pause();
      return result;
    };
  });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/#/settings');
  await page.getByRole('checkbox', { name: /减少动态效果/ }).uncheck();
  await page.getByRole('button', { name: '保存设置' }).click();
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.locator('dialog.lyrics-view');
  await expect(sheet).toHaveAttribute('data-phase', 'opening');
  expect(await sheet.evaluate(el => (el as HTMLDialogElement).open)).toBe(true);
  await expect(page.locator('.sidebar')).toHaveAttribute('inert', '');
  const entry = await sheet.evaluate(el => {
    const animation = el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!;
    animation.currentTime = 180;
    const keys = (animation.effect as KeyframeEffect).getKeyframes();
    return { transform: getComputedStyle(el).transform, opacity: getComputedStyle(el).opacity, keys };
  });
  expect(entry.transform).not.toBe('none');
  expect(Number(entry.opacity)).toBeGreaterThan(0);
  expect(Number(entry.opacity)).toBeLessThan(1);
  expect(entry.keys.every(key => !('borderRadius' in key) && !String(key.transform).includes('scale'))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(sheet).toHaveAttribute('data-phase', 'closing');
  expect(await sheet.evaluate(el => getComputedStyle(el).transform)).toBe(entry.transform);
  await page.locator('.player-bar .art-button').evaluate(el => (el as HTMLButtonElement).click());
  await expect(sheet).toHaveAttribute('data-phase', 'opening');
  await sheet.evaluate(el => el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.finish());
  await expect(sheet).toHaveAttribute('data-phase', 'open');
  await sheet.getByRole('button', { name: '关闭歌词' }).click();
  await expect(sheet).toHaveAttribute('data-phase', 'closing');
  expect(await sheet.evaluate(el => (el as HTMLDialogElement).open)).toBe(true);
  await expect(page.locator('.player-bar')).toHaveAttribute('inert', '');
  const closingTransform = await sheet.evaluate(el => {
    el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.currentTime = 160;
    return getComputedStyle(el).transform;
  });
  expect(closingTransform).not.toBe('none');
  // A new open request during exit reverses the same dialog from its current pose.
  await page.locator('.player-bar .art-button').evaluate(el => (el as HTMLButtonElement).click());
  await expect(sheet).toHaveCount(1);
  await expect(sheet).toHaveAttribute('data-phase', 'opening');
  expect(await sheet.evaluate(el => getComputedStyle(el).transform)).toBe(closingTransform);
  await sheet.evaluate(el => el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.finish());
  await expect(sheet).toHaveAttribute('data-phase', 'open');
  await page.keyboard.press('Control+f');
  await expect(sheet).toHaveAttribute('data-phase', 'closing');
  await expect(page.locator('.workspace')).toHaveAttribute('inert', '');
  await sheet.evaluate(el => el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.finish());
  await expect(sheet).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: '搜索音乐' })).toBeFocused();
  await expect(page.locator('.workspace')).not.toHaveAttribute('inert');
});

test('changing reduced motion during sheet exit finishes closing without trapping focus', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/#/settings');
  await page.getByRole('checkbox', { name: /减少动态效果/ }).uncheck();
  await page.getByRole('button', { name: '保存设置' }).click();
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.locator('dialog.lyrics-view');
  await expect(sheet).toHaveAttribute('data-phase', 'open');
  await page.evaluate(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args: Parameters<Element['animate']>) {
      const result = animate.apply(this, args);
      if (this.classList.contains('lyrics-view')) result.pause();
      return result;
    };
  });
  await page.keyboard.press('Escape');
  await expect(sheet).toHaveAttribute('data-phase', 'closing');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(sheet).toHaveCount(0);
  await expect(page.getByRole('button', { name: '歌词', exact: true })).toBeFocused();
});

test('closing before the lazy player module loads does not leave the application inert', async ({ page }) => {
  let release!: () => void;
  const pending = new Promise<void>(resolve => { release = resolve; });
  let requested = false;
  await page.route('**/web/components/Lyrics.vue*', async route => {
    requested = true;
    await pending;
    await route.continue();
  });
  try {
    await page.goto('/');
    await page.getByRole('button', { name: '歌词', exact: true }).click();
    await expect.poll(() => requested).toBe(true);
    await page.keyboard.press('Control+f');
    release();
    await expect(page.getByRole('textbox', { name: '搜索音乐' })).toBeFocused();
    await expect(page.locator('dialog.lyrics-view')).toHaveCount(0);
    await expect(page.locator('.workspace')).not.toHaveAttribute('inert');
  } finally { release(); }
});

test('background has a fixed pixel budget, cached preblur and no work during sheet transitions', async ({ page }) => {
  await page.addInitScript(() => {
    (window as any).backgroundDraws = 0;
    (window as any).textureReads = 0;
    const draw = CanvasRenderingContext2D.prototype.drawImage;
    CanvasRenderingContext2D.prototype.drawImage = function (...args: any[]) {
      if ((this.canvas as HTMLCanvasElement).classList?.contains('album-background-canvas')) (window as any).backgroundDraws++;
      return (draw as any).apply(this, args);
    };
    const read = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function (...args: any[]) {
      if (this.canvas.width === 48) (window as any).textureReads++;
      return (read as any).apply(this, args);
    };
  });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/#/settings');
  await page.getByRole('checkbox', { name: /减少动态效果/ }).uncheck();
  await page.getByRole('button', { name: '保存设置' }).click();
  await page.evaluate(async () => {
    const cover = document.createElement('canvas'); cover.width = cover.height = 96;
    const ctx = cover.getContext('2d')!;
    const gradient = ctx.createLinearGradient(0, 0, 96, 96);
    gradient.addColorStop(0, '#ff986b'); gradient.addColorStop(1, '#42648d');
    ctx.fillStyle = gradient; ctx.fillRect(0, 0, 96, 96);
    const ipc = (window as any).__TAURI_INTERNALS__;
    const state = await ipc.invoke('player_snapshot');
    state.queue[0].cover = cover.toDataURL();
    await ipc.invoke('player_command', { command: { type: 'replace', tracks: state.queue, index: 0 } });
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args: Parameters<Element['animate']>) {
      const result = animate.apply(this, args);
      if (this.classList.contains('lyrics-view')) result.pause();
      return result;
    };
  });
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.locator('dialog.lyrics-view');
  await expect(sheet).toHaveAttribute('data-phase', 'opening');
  expect(await page.evaluate(() => (window as any).textureReads)).toBe(0);
  expect(await page.evaluate(() => (window as any).backgroundDraws)).toBe(0);
  await sheet.evaluate(el => el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.finish());
  await expect(sheet.locator('.album-background-canvas')).toHaveClass(/ready/);
  await expect(sheet.locator('.album-background')).toHaveAttribute('data-motion', 'flowing');
  const budget = await sheet.locator('canvas').evaluate(el => ({ width: (el as HTMLCanvasElement).width, height: (el as HTMLCanvasElement).height, filter: getComputedStyle(el).filter }));
  expect(budget.width).toBeLessThanOrEqual(480);
  expect(budget.height).toBeLessThanOrEqual(320);
  expect(budget.filter).toBe('none');
  const frames = await page.evaluate(async () => {
    const start = (window as any).backgroundDraws;
    await new Promise(resolve => setTimeout(resolve, 1100));
    return ((window as any).backgroundDraws - start) / 2;
  });
  expect(frames).toBeGreaterThan(0);
  expect(frames).toBeLessThanOrEqual(28);
  expect(await page.evaluate(() => (window as any).textureReads)).toBe(1);
  await page.keyboard.press('Escape');
  await expect(sheet).toHaveAttribute('data-phase', 'closing');
  const frozen = await page.evaluate(async () => {
    const start = (window as any).backgroundDraws;
    const progress = (document.querySelector('dialog input[aria-label="播放进度"]') as HTMLInputElement).value;
    await new Promise(resolve => setTimeout(resolve, 150));
    return { draws: (window as any).backgroundDraws - start, unchanged: progress === (document.querySelector('dialog input[aria-label="播放进度"]') as HTMLInputElement).value };
  });
  expect(frozen).toEqual({ draws: 0, unchanged: true });
  await sheet.evaluate(el => el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.finish());
  await expect(sheet).toHaveCount(0);
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  await expect(sheet).toHaveAttribute('data-phase', 'opening');
  await sheet.evaluate(el => el.getAnimations().find(a => (a.effect as KeyframeEffect).target === el)!.finish());
  await expect(sheet.locator('.album-background-canvas')).toHaveClass(/ready/);
  expect(await page.evaluate(() => (window as any).textureReads)).toBe(1);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(sheet.locator('.album-background')).toHaveAttribute('data-motion', 'static');
});

test('long lyrics cull offscreen paint and restore rows when scrolling or resizing', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.connection .online')).toBeVisible();
  await page.evaluate(() => {
    const ipc = (window as any).__TAURI_INTERNALS__, original = ipc.invoke;
    ipc.invoke = (cmd: string, args: any) => cmd === 'lyrics' ? Promise.resolve(Array.from({ length: 400 }, (_, i) => ({
      time_ms: i * 1000, text: `歌词 ${i} — 很长的一句歌词需要正常换行显示`, translation: i % 3 ? '' : 'Translation with variable height',
    }))) : original(cmd, args);
  });
  await page.getByRole('button', { name: '歌词', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: '正在播放' });
  await expect(sheet.locator('[data-line]')).toHaveCount(400);
  await expect.poll(() => sheet.locator('[data-line]:not([data-offscreen=true])').count()).toBeLessThan(30);
  await expect(sheet.locator('[data-line="399"]')).toHaveCSS('content-visibility', 'hidden');
  await expect(sheet.locator('[data-line="399"]')).toHaveCSS('filter', 'none');
  await sheet.locator('.lyric-lines').hover();
  await page.mouse.wheel(0, 1500);
  await expect(sheet.getByRole('button', { name: '返回当前歌词' })).toBeVisible();
  await sheet.getByRole('button', { name: '返回当前歌词' }).click();
  await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('player_command', { command: { type: 'seek', position_ms: 179000 } }));
  await expect(sheet.locator('[data-line="179"]')).not.toHaveAttribute('data-offscreen', 'true');
  await page.setViewportSize({ width: 600, height: 700 });
  await expect.poll(async () => sheet.locator('.lyric-lines').evaluate(el => {
    const active = el.querySelector('.active')!.getBoundingClientRect();
    return Math.abs((active.top + active.height / 2 - el.getBoundingClientRect().top) / el.clientHeight - .44);
  })).toBeLessThan(.02);
  expect(await sheet.locator('[data-line]:not([data-offscreen=true])').count()).toBeLessThan(30);
});

test('integrated titlebar controls and close preserve native lifecycle', async ({ page }) => {
  await page.goto('/');
  const controls = page.locator('.topbar').getByRole('group', { name: '窗口控制' });
  await expect(controls).toBeVisible();
  await expect(page.locator('.topbar')).toHaveAttribute('data-tauri-drag-region', '');
  await controls.getByRole('button', { name: '最大化窗口', exact: true }).click();
  await expect(controls.getByRole('button', { name: '还原窗口' })).toBeVisible();
  await controls.getByRole('button', { name: '还原窗口' }).click();
  await expect(controls.getByRole('button', { name: '最大化窗口', exact: true })).toBeVisible();
  await controls.getByRole('button', { name: '最小化窗口' }).click();
  await page.getByRole('textbox', { name: '搜索音乐' }).fill('标题栏搜索');
  await page.getByRole('textbox', { name: '搜索音乐' }).press('Enter');
  await expect(page).toHaveURL(/search_song/);
  await controls.getByRole('button', { name: '关闭窗口' }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__windowCommands)).toContain('plugin:window|destroy');
  const commands = await page.evaluate(() => (window as any).__windowCommands as string[]);
  expect(commands).toContain('plugin:window|minimize');
  expect(commands).toContain('plugin:window|close');
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem('test-ui')!).route)).toContain('search_song');
});

test('window buttons fit at minimum width and remain available in lyrics', async ({ page }) => {
  await page.setViewportSize({ width: 760, height: 560 });
  await page.goto('/');
  const close = page.locator('.topbar').getByRole('button', { name: '关闭窗口' });
  await expect(close).toBeInViewport();
  const search = await page.getByRole('textbox', { name: '搜索音乐' }).boundingBox();
  expect(search!.width).toBeGreaterThan(40);
  await page.getByRole('button', { name: '打开歌词', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: '正在播放' });
  await expect(dialog.getByRole('button', { name: '关闭窗口' })).toBeInViewport();
  await dialog.getByRole('button', { name: '最大化窗口', exact: true }).click();
  await expect(dialog.getByRole('button', { name: '还原窗口' })).toBeVisible();
});
