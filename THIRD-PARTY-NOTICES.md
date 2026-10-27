# Third-party notices

The desktop application is licensed under GPL-3.0-or-later, consistent with the original Rust project. See [COPYING](COPYING).

The queue, session compatibility, playback setup and desktop integration are derived from [gmg137/netease-cloud-music-gtk](https://github.com/gmg137/netease-cloud-music-gtk), copyright gmg137 and contributors, under GPL-3.0-or-later. Relevant source files retain attribution. The standalone Git history begins with the current Tauri application; removing legacy GTK files and upstream commits does not remove attribution or change the license of derived code.

[YesPlayMusic](https://github.com/qier222/YesPlayMusic) 0.4.11 (MIT, copyright 2020–2023 qier222) was consulted as a design and interaction reference. The current Vue 3 components were newly implemented; no YesPlayMusic runtime, bundled font or icon assets are included. Apple Music and Spotify were also used as visual references.

The immersive now-playing page uses Apple Music-like Lyrics (AMLL, https://github.com/amll-dev/applemusic-like-lyrics) as a visual reference only. Its lyric scrolling, background layers and playback layout are independently implemented. No AMLL source code or runtime dependency is included. AMLL was evaluated and removed after discovering that its package license text is AGPL despite conflicting npm metadata.

Windows installers are configured to include the official GStreamer 1.26.11 MSVC x64 runtime and plugins. The staging script retains the SDK's `share/licenses`/documentation notices in `runtime/gstreamer/share/`, and records DLL hashes in `runtime-manifest.json`. GStreamer and bundled codec libraries have their own LGPL/GPL/permissive terms; copying notices alone does not discharge source-provision or other distribution obligations. Audit the exact staged plugins and provide corresponding sources/offers before public distribution. Windows also uses app-local Microsoft VC Runtime redistributables under Microsoft's redistribution terms, WebView2 under its own terms, and a statically built OpenSSL dependency whose notices/source obligations must be included in release materials.

`src-tauri/protocols/gtk-shell.xml` is copied from GTK's `gtk-3-24` branch (`gdk/wayland/protocol/gtk-shell.xml`, https://github.com/GNOME/gtk), under GTK's LGPL-2.1-or-later licensing. It generates the small client binding used to complete GNOME startup notifications without opening a window. The Rust Wayland client/scanner/server crates retain their MIT licenses.

Icons are provided by `lucide-vue-next` (ISC). Vue, Vue Router, Vite and Tauri and their transitive dependencies retain their respective licenses, available with the dependency packages. Debian packages use system GStreamer and WebKitGTK; AppImage bundles include these libraries and their package copyright notices under `usr/share/doc`.
