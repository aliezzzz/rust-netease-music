#!/usr/bin/env python3
"""Real WebKit/Tauri IPC regression, with isolated state and a local WAV source.

Run within dbus-run-session. NCM_TEST_BINARY selects an extracted package binary.
Requires Xvfb, tauri-driver and WebKitWebDriver on PATH. No account is used.
"""
import base64
import json
import os
from pathlib import Path
import select
import signal
import socket
import struct
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import wave
from gi.repository import Gio, GLib

workspace = Path(__file__).resolve().parents[1]
binary = Path(os.environ.get('NCM_TEST_BINARY', workspace / 'target/release/netease-cloud-music-desktop')).resolve()
output = Path(os.environ.get('NCM_TEST_OUTPUT') or tempfile.mkdtemp(prefix='ncm-webdriver-'))
output.mkdir(parents=True, exist_ok=True)
root = output / 'data'
(root / 'config').mkdir(parents=True, exist_ok=True)
tone = output / 'tone.wav'
with wave.open(str(tone), 'wb') as f:
    f.setnchannels(1)
    f.setsampwidth(2)
    f.setframerate(8000)
    f.writeframes(struct.pack('<h', 0) * 8000 * 30)
tracks = [dict(id=i, name=f'原生测试 {i}', artist='WebKit 集成', source=tone.as_uri(), duration_ms=30000, playable=True) for i in range(1, 4)]
(root / 'config/state.json').write_text(json.dumps(dict(version=1, queue=dict(tracks=tracks, current=1, repeat='all'), settings=dict(theme='light'))))
env = dict(os.environ, NCM_DATA_DIR=str(root), NCM_AUDIO_SINK='fakesink', GDK_BACKEND='x11', WEBKIT_DISABLE_DMABUF_RENDERER='1')
live = not os.environ.get('NCM_TEST_OFFLINE')

def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]

port, native_port = free_port(), free_port()
session = None
driver = None
watcher = subprocess.Popen(['python3', str(workspace / 'scripts/tray-watcher.py'), str(output / 'tray.json')], env=env)
read_fd, write_fd = os.pipe()
log = (output / 'driver.log').open('w')
xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(write_fd), '-screen', '0', '1280x840x24', '-nolisten', 'tcp', '-ac'], pass_fds=(write_fd,), stdout=log, stderr=log, env=env)
os.close(write_fd)

def request(method, path, body=None):
    req = urllib.request.Request(f'http://127.0.0.1:{port}{path}', data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json'}, method=method)
    try:
        with urllib.request.urlopen(req, timeout=45) as response:
            result = json.load(response).get('value')
    except urllib.error.HTTPError as e:
        raise RuntimeError(e.read().decode()) from e
    if isinstance(result, dict) and 'error' in result:
        raise RuntimeError(result)
    return result

def execute(script, *args):
    return request('POST', f'/session/{session}/execute/sync', dict(script=script, args=args))

def until(predicate, label):
    start = time.monotonic()
    while time.monotonic() - start < 30:
        if predicate():
            return
        time.sleep(.15)
    raise RuntimeError('Timed out: ' + label)

def click(selector):
    element = request('POST', f'/session/{session}/element', dict(using='css selector', value=selector))
    key = next(iter(element.values()))
    request('POST', f'/session/{session}/element/{key}/click', {})

def invoke(command, args=None):
    result = request('POST', f'/session/{session}/execute/async', dict(script="const done = arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]).then(v => done({ok:true,value:v})).catch(e => done({ok:false,error:String(e)}));", args=[command, args or {}]))
    if not result['ok']:
        raise RuntimeError(result['error'])
    return result.get('value')

def screenshot(name):
    image = request('GET', f'/session/{session}/screenshot')
    (output / name).write_bytes(base64.b64decode(image))

def tray_call(path, interface, method, parameters):
    tray = json.loads((output / 'tray.json').read_text())
    connection = Gio.bus_get_sync(Gio.BusType.SESSION)
    return connection.call_sync(tray['service'], path or tray['path'], interface, method, parameters, None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

def tray_action(label):
    menu_path = tray_call(None, 'org.freedesktop.DBus.Properties', 'Get', GLib.Variant('(ss)', ('org.kde.StatusNotifierItem', 'Menu')))[0]
    layout = tray_call(menu_path, 'com.canonical.dbusmenu', 'GetLayout', GLib.Variant('(iias)', (0, -1, ['label'])))[1]
    def find(node):
        if hasattr(node, 'unpack'): node = node.unpack()
        item, props, children = node
        if props.get('label') == label: return item
        for child in children:
            value = find(child)
            if value is not None: return value
    item = find(layout)
    if item is None: raise RuntimeError('Tray item missing: ' + label)
    tray_call(menu_path, 'com.canonical.dbusmenu', 'Event', GLib.Variant('(isvu)', (item, 'clicked', GLib.Variant('s', ''), 0)))

def mpris(method, parameters):
    return Gio.bus_get_sync(Gio.BusType.SESSION).call_sync('org.mpris.MediaPlayer2.NeteaseCloudMusicDesktop', '/org/mpris/MediaPlayer2', 'org.mpris.MediaPlayer2.Player', method, parameters, None, Gio.DBusCallFlags.NONE, 5000, None)

try:
    if not select.select([read_fd], [], [], 15)[0]:
        raise RuntimeError('Xvfb startup timeout')
    env['DISPLAY'] = ':' + os.read(read_fd, 100).decode().strip()
    driver = subprocess.Popen(['tauri-driver', '--port', str(port), '--native-port', str(native_port)], env=env, stdout=log, stderr=log, start_new_session=True)
    def listening():
        try:
            request('GET', '/status')
            return True
        except (OSError, RuntimeError):
            return False
    until(listening, 'WebDriver startup')
    session = request('POST', '/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': str(binary)}}}})['sessionId']
    request('POST', f'/session/{session}/timeouts', {'script': 30000, 'implicit': 1000})
    until(lambda: execute("return !!document.querySelector('.connection .online')"), 'real IPC initialization')
    if live:
        until(lambda: execute("return document.querySelectorAll('.music-card').length > 0"), 'live discovery data')
        screenshot('discover.png')
    click('[aria-label="播放"]')
    until(lambda: invoke('player_snapshot')['playback'] == 'playing', 'UI starts real GStreamer')
    click('[aria-label="播放队列"]')
    until(lambda: execute("return document.querySelectorAll('.queue-track').length === 3"), 'queue renders')
    click('.queue-item:nth-child(2) .queue-track')
    until(lambda: invoke('player_snapshot')['current_id'] == 2, 'queue selection IPC')
    click('[aria-label="暂停"]')
    until(lambda: invoke('player_snapshot')['playback'] == 'paused', 'pause')
    mpris('SetPosition', GLib.Variant('(ox)', ('/net/netease/track/2', 12000000)))
    until(lambda: abs(invoke('player_snapshot')['position_ms'] - 12000) < 1000, 'seek')
    screenshot('queue.png')
    if live:
        execute("location.hash = '/search_song?q=' + encodeURIComponent('晴天')")
        until(lambda: execute("return document.querySelectorAll('.track-row').length > 0"), 'live search')
        until(lambda: execute("return Array.from(document.querySelectorAll('.track-row img')).some(img => img.naturalWidth > 0)"), 'live search cover loads')
        screenshot('search.png')
        click('.account-button')
        until(lambda: execute("return document.querySelector('.qr-image img')?.naturalWidth > 0"), 'live login QR')
        screenshot('login.png')
        click('[aria-label="关闭登录"]')
    execute("location.hash = '/settings'")
    until(lambda: execute("return !!document.querySelector('.settings-page')"), 'settings')
    execute("const el = document.querySelector('select'); el.value = 'dark'; el.dispatchEvent(new Event('change', {bubbles:true}))")
    click('.settings-page form button[type="submit"], .settings-page form > .primary-button')
    until(lambda: invoke('settings')['theme'] == 'dark', 'settings persist')
    screenshot('settings-dark.png')
    if os.environ.get('NCM_TEST_NETWORK'):
        songs = invoke('browse', {'query': {'kind': 'search_song', 'text': '晴天', 'limit': 6}})['tracks']
        played = False
        for track in songs:
            if not track['playable']: continue
            invoke('player_command', {'command': {'type': 'replace', 'tracks': [track], 'index': 0}})
            until(lambda: (s := invoke('player_snapshot'))['playback'] == 'error' or (s['playback'] == 'playing' and s['position_ms'] >= 2000), 'network decode')
            if invoke('player_snapshot')['playback'] == 'playing':
                print(f"NETWORK_OK: {track['id']} {invoke('player_snapshot')['duration_ms']}ms")
                played = True
                break
        if not played: raise RuntimeError('No live audio decoded')
        screenshot('network.png')
        if os.environ.get('NCM_TEST_CACHE'):
            # Stop closes the GStreamer writer before the atomic cache commit.
            # Replay with an unreachable API proxy proves the cached file is used.
            time.sleep(3)
            invoke('player_command', {'command': {'type': 'stop'}})
            cached = root / 'cache' / f"music_{track['id']}_128000"
            if not cached.is_file(): raise RuntimeError('Network download did not produce a complete cache')
            settings = invoke('settings')
            invoke('save_settings', {'settings': dict(settings, proxy='http://127.0.0.1:1')})
            invoke('player_command', {'command': {'type': 'play'}})
            until(lambda: (s := invoke('player_snapshot'))['playback'] == 'playing' and s['position_ms'] >= 1000, 'cached audio replay with unreachable API')
            invoke('player_command', {'command': {'type': 'seek', 'position_ms': max(0, track['duration_ms'] - 4000)}})
            until(lambda: invoke('player_snapshot')['position_ms'] >= track['duration_ms'] - 5000, 'cached tail seek')
            invoke('save_settings', {'settings': settings})
            print(f'CACHE_REPLAY_OK: {cached.stat().st_size} bytes')
        invoke('player_command', {'command': {'type': 'replace', 'tracks': tracks, 'index': 1}})
        invoke('player_command', {'command': {'type': 'pause'}})
        until(lambda: invoke('player_snapshot')['playback'] == 'paused', 'restore local queue')
    until(lambda: (output / 'tray.json').exists(), 'StatusNotifier registration')
    tray_service = json.loads((output / 'tray.json').read_text())['service']
    app_pid = Gio.bus_get_sync(Gio.BusType.SESSION).call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', GLib.Variant('(s)', (tray_service,)), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    tray_action('播放')
    until(lambda: invoke('player_snapshot')['playback'] == 'playing', 'tray play')
    tray_action('暂停')
    until(lambda: invoke('player_snapshot')['playback'] == 'paused', 'tray pause')
    tray_call(None, 'org.kde.StatusNotifierItem', 'Activate', GLib.Variant('(ii)', (0, 0)))
    # Verify actual queue and preferences remain persisted after orderly quit.
    # Exiting may terminate WebDriver before the final JS callback is delivered.
    try:
        tray_action('退出')
    except RuntimeError as error:
        if 'Session terminated without a reply' not in str(error):
            raise
    def application_exited():
        return not Path(f'/proc/{app_pid}/exe').exists()
    until(application_exited, 'application process exits')
    until(lambda: (root / 'config/state.json').exists() and json.loads((root / 'config/state.json').read_text())['settings']['theme'] == 'dark', 'shutdown persistence')
    print('NATIVE_WEBDRIVER_OK: playback, queue, pause, MPRIS seek, settings, tray play/pause/activate/quit' + (', live discovery/search/covers/QR' if live else ''))
    print('Artifacts:', output)
finally:
    watcher.terminate()
    watcher.wait(timeout=10)
    if session:
        try:
            request('DELETE', f'/session/{session}')
        except (OSError, RuntimeError):
            pass
    if driver:
        try:
            os.killpg(driver.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        driver.wait(timeout=10)
    os.close(read_fd)
    xvfb.terminate()
    xvfb.wait(timeout=10)
    log.close()
