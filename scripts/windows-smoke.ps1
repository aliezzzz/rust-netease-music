# Validate an app-local runtime without the GStreamer SDK on PATH.
# Build first: cargo build -p netease-cloud-music-desktop --features native-smoke,custom-protocol
param([string]$Binary = "$PSScriptRoot\..\target\debug\netease-cloud-music-desktop.exe")
$ErrorActionPreference = 'Stop'
$desktop = Split-Path $PSScriptRoot -Parent
$stage = Join-Path $desktop 'src-tauri\windows-runtime'
if (!(Test-Path $Binary)) { throw "Missing smoke binary: $Binary" }
if (!(Test-Path "$stage\bin\gstreamer-1.0-0.dll")) { throw 'Run stage-windows-runtime.ps1 first.' }
$root = Join-Path $env:TEMP ("ncm-windows-smoke-" + [guid]::NewGuid().ToString('N'))
$app = Join-Path $root 'app'
New-Item "$app\runtime" -ItemType Directory -Force | Out-Null
Copy-Item $Binary "$app\netease-cloud-music-desktop.exe"
Copy-Item "$stage\bin\*.dll" $app
Copy-Item "$stage\gstreamer" "$app\runtime" -Recurse
# Diagnostics live beside the app DLLs but are not included in installers.
foreach ($tool in @('gst-inspect-1.0.exe', 'gst-launch-1.0.exe')) {
    Copy-Item "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\bin\$tool" $app
}
# The native test uses two six-second WAV tracks, a silent audio sink, actual
# WebView2 IPC, single-instance recovery and 30 close/recreate cycles.
$audio = Join-Path $root 'tone.wav'
$writer = [IO.BinaryWriter]::new([IO.File]::Create($audio))
try {
    $samples = 6 * 8000
    $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF')); $writer.Write([int](36 + $samples * 2))
    $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt ')); $writer.Write([int]16)
    $writer.Write([int16]1); $writer.Write([int16]1); $writer.Write([int]8000); $writer.Write([int]16000)
    $writer.Write([int16]2); $writer.Write([int16]16)
    $writer.Write([Text.Encoding]::ASCII.GetBytes('data')); $writer.Write([int]($samples * 2))
    for ($i = 0; $i -lt $samples; $i++) { $writer.Write([int16](5000 * [Math]::Sin($i * 440 * 2 * [Math]::PI / 8000))) }
} finally { $writer.Dispose() }
$keys = @('PATH', 'GST_PLUGIN_PATH', 'GST_PLUGIN_PATH_1_0', 'GST_PLUGIN_SYSTEM_PATH', 'GST_PLUGIN_SYSTEM_PATH_1_0', 'GST_PLUGIN_SCANNER', 'GST_PLUGIN_SCANNER_1_0', 'GST_REGISTRY_1_0', 'GIO_MODULE_DIR', 'GIO_EXTRA_MODULES', 'SSL_CERT_FILE', 'NCM_DATA_DIR', 'NCM_AUDIO_SINK', 'NCM_NATIVE_SMOKE', 'NCM_NATIVE_SMOKE_AUDIO', 'NCM_NATIVE_SMOKE_REPORT')
$saved = @{}
foreach ($key in $keys) { $saved[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
$report = Join-Path $root 'result.txt'
try {
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $null, 'Process') }
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $env:NCM_DATA_DIR = Join-Path $root 'state'
    $env:NCM_AUDIO_SINK = 'fakesink'
    $env:NCM_NATIVE_SMOKE = '1'
    $env:NCM_NATIVE_SMOKE_AUDIO = $audio
    $env:NCM_NATIVE_SMOKE_REPORT = $report
    $env:GST_PLUGIN_SYSTEM_PATH_1_0 = "$app\runtime\gstreamer\lib\gstreamer-1.0"
    $env:GST_PLUGIN_SCANNER_1_0 = "$app\runtime\gstreamer\libexec\gstreamer-1.0\gst-plugin-scanner.exe"
    $env:GST_REGISTRY_1_0 = "$root\codec-registry.bin"
    $env:GIO_MODULE_DIR = "$app\runtime\gstreamer\lib\gio\modules"
    $env:SSL_CERT_FILE = "$app\runtime\gstreamer\etc\ssl\certs\ca-certificates.crt"
    # Scanner is a child executable, so its dependent DLLs need the app-local PATH.
    $env:PATH = "$app;$env:PATH"
    foreach ($decoder in @('avdec_aac', 'avdec_alac', 'mpg123audiodec', 'flacdec', 'wasapisink', 'souphttpsrc')) {
        & "$app\gst-inspect-1.0.exe" $decoder *> "$root\$decoder.log"
        if ($LASTEXITCODE) { throw "Bundled plugin check failed: $decoder; logs: $root" }
    }
    $encoded = Join-Path $root 'aac.m4a'
    & "$app\gst-launch-1.0.exe" -q audiotestsrc num-buffers=20 '!' audioconvert '!' audioresample '!' 'audio/x-raw,rate=44100,channels=2' '!' avenc_aac '!' mp4mux '!' filesink "location=$encoded" *> "$root\aac-encode.log"
    if ($LASTEXITCODE) { throw "Bundled AAC encode failed; logs: $root" }
    $uri = ([Uri]::new($encoded)).AbsoluteUri
    & "$app\gst-launch-1.0.exe" -q playbin3 "uri=$uri" 'audio-sink=fakesink' 'video-sink=fakesink' *> "$root\aac-decode.log"
    if ($LASTEXITCODE) { throw "Bundled AAC decode failed; logs: $root" }
    # Clear overrides again: the app must locate plugins itself.
    foreach ($key in @('GST_PLUGIN_SYSTEM_PATH_1_0', 'GST_PLUGIN_SCANNER_1_0', 'GST_REGISTRY_1_0', 'GIO_MODULE_DIR', 'SSL_CERT_FILE')) {
        [Environment]::SetEnvironmentVariable($key, $null, 'Process')
    }
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $process = Start-Process "$app\netease-cloud-music-desktop.exe" -PassThru -RedirectStandardOutput "$root\stdout.log" -RedirectStandardError "$root\stderr.log"
    if (!$process.WaitForExit(300000)) { $process.Kill(); throw "Smoke timed out. Logs: $root" }
    $process.Refresh()
    if ($process.ExitCode -ne 0 -or !(Test-Path $report) -or (Get-Content $report -Raw).Trim() -ne 'PASS') {
        throw "Windows native smoke failed. Logs: $root"
    }
    Write-Host "PASS: isolated Windows native lifecycle smoke. Logs: $root"
} finally {
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key], 'Process') }
    if ($env:GITHUB_ENV) { "NCM_WINDOWS_SMOKE_LOGS=$root" | Out-File $env:GITHUB_ENV -Append -Encoding utf8 }
}
