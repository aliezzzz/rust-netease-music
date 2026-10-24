//! Packaged Windows GStreamer discovery. The loader needs core DLLs beside the
//! EXE *before* main runs; stage-windows-runtime.ps1 enforces that layout.
#[cfg(any(windows, test))]
use std::path::{Path, PathBuf};

#[cfg(any(windows, test))]
fn bundled_layout(executable: &Path) -> Option<(PathBuf, PathBuf)> {
    let directory = executable.parent()?;
    let runtime = directory.join("runtime").join("gstreamer");
    let plugins = runtime.join("lib").join("gstreamer-1.0");
    let scanner = runtime
        .join("libexec")
        .join("gstreamer-1.0")
        .join("gst-plugin-scanner.exe");
    (directory.join("gstreamer-1.0-0.dll").is_file()
        && plugins.join("gstcoreelements.dll").is_file()
        && plugins.join("gstlibav.dll").is_file()
        && scanner.is_file())
    .then_some((plugins, scanner))
}

pub fn prepare() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()?;
        if let Some((plugins, scanner)) = bundled_layout(&executable) {
            // Do not mix the packaged runtime with a globally installed SDK.
            std::env::set_var("GST_PLUGIN_PATH", "");
            std::env::set_var("GST_PLUGIN_PATH_1_0", "");
            std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", plugins);
            std::env::set_var("GST_PLUGIN_SCANNER_1_0", scanner);
            let directory = executable.parent().unwrap();
            let runtime = directory.join("runtime").join("gstreamer");
            let gio = runtime.join("lib/gio/modules");
            let certificates = runtime.join("etc/ssl/certs/ca-certificates.crt");
            anyhow::ensure!(
                gio.join("libgioopenssl.dll").is_file() && certificates.is_file(),
                "安装包缺少 HTTPS/TLS 模块或证书文件，请重新安装完整 Windows 版本"
            );
            std::env::set_var("GIO_MODULE_DIR", &gio);
            std::env::set_var("GIO_EXTRA_MODULES", &gio);
            std::env::set_var("SSL_CERT_FILE", certificates);
            let mut paths = vec![directory.to_path_buf()];
            if let Some(path) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&path));
            }
            std::env::set_var("PATH", std::env::join_paths(paths)?);
            // Isolate plugin blacklists from developer/system installations.
            let paths = music_core::Paths::discover()?;
            std::env::set_var(
                "GST_REGISTRY_1_0",
                paths.cache.join("windows-gst-registry.bin"),
            );
        } else if !cfg!(debug_assertions) {
            anyhow::bail!(
                "安装包缺少 GStreamer DLL、libav 插件或插件扫描程序，请重新安装完整 Windows 版本"
            );
        }
        // Development runs use the explicitly configured MSVC GStreamer SDK.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_layout_requires_decoders_and_scanner_next_to_executable() {
        let root = std::env::temp_dir().join(format!("ncm-runtime-layout-{}", std::process::id()));
        let executable = root.join("netease-cloud-music-desktop.exe");
        let plugins = root.join("runtime/gstreamer/lib/gstreamer-1.0");
        let scanner = root.join("runtime/gstreamer/libexec/gstreamer-1.0/gst-plugin-scanner.exe");
        std::fs::create_dir_all(&plugins).unwrap();
        std::fs::create_dir_all(scanner.parent().unwrap()).unwrap();
        assert!(bundled_layout(&executable).is_none());
        for file in [
            root.join("gstreamer-1.0-0.dll"),
            plugins.join("gstcoreelements.dll"),
            plugins.join("gstlibav.dll"),
            scanner.clone(),
        ] {
            std::fs::write(file, b"fixture").unwrap();
        }
        assert_eq!(bundled_layout(&executable), Some((plugins, scanner)));
        std::fs::remove_dir_all(root).unwrap();
    }
}
