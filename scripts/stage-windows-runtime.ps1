# Stage resources before `npm run tauri build`. Never copy SDK import libraries
# as runtime files: core DLLs must be installed beside the EXE for the OS loader.
param([string]$GStreamerRoot = $env:GSTREAMER_1_0_ROOT_MSVC_X86_64)
$ErrorActionPreference = 'Stop'
if (!$GStreamerRoot) { throw 'Set GSTREAMER_1_0_ROOT_MSVC_X86_64 to the x64 MSVC GStreamer SDK.' }
$desktop = Split-Path $PSScriptRoot -Parent
$stage = Join-Path $desktop 'src-tauri\windows-runtime'
$gst = Join-Path $stage 'gstreamer'
$bin = Join-Path $stage 'bin'
foreach ($file in @('lib\gstreamer-1.0.lib', 'bin\gstreamer-1.0-0.dll', 'lib\gstreamer-1.0\gstcoreelements.dll', 'lib\gstreamer-1.0\gstlibav.dll', 'libexec\gstreamer-1.0\gst-plugin-scanner.exe', 'lib\gio\modules\libgioopenssl.dll', 'etc\ssl\certs\ca-certificates.crt')) {
    if (!(Test-Path (Join-Path $GStreamerRoot $file))) { throw "Missing SDK runtime file: $file" }
}
# Check architecture, so an x86 / MinGW SDK cannot silently produce a broken bundle.
$bytes = [IO.File]::ReadAllBytes((Join-Path $GStreamerRoot 'bin\gstreamer-1.0-0.dll'))
$pe = [BitConverter]::ToInt32($bytes, 0x3c)
if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664) { throw 'GStreamer must be x64.' }
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item $bin -ItemType Directory -Force | Out-Null
New-Item "$gst\lib" -ItemType Directory -Force | Out-Null
Copy-Item "$GStreamerRoot\bin\*.dll" $bin
Copy-Item "$GStreamerRoot\lib\gstreamer-1.0" "$gst\lib" -Recurse
Copy-Item "$GStreamerRoot\libexec" $gst -Recurse
Copy-Item "$GStreamerRoot\lib\gio" "$gst\lib" -Recurse
Copy-Item "$GStreamerRoot\etc" $gst -Recurse
# Include official SDK copyright/license notices with the distributed runtime.
New-Item "$gst\share" -ItemType Directory -Force | Out-Null
foreach ($directory in @('licenses', 'doc', 'glib-2.0')) {
    if (Test-Path "$GStreamerRoot\share\$directory") { Copy-Item "$GStreamerRoot\share\$directory" "$gst\share" -Recurse }
}
$noticeFiles = @(Get-ChildItem "$GStreamerRoot\*" -File | Where-Object { $_.Name -match '^(COPYING|LICENSE|NOTICE)' })
foreach ($file in $noticeFiles) { Copy-Item $file.FullName $gst }
if (!(Test-Path "$gst\share\licenses") -and !(Test-Path "$gst\share\doc") -and !$noticeFiles.Count) {
    throw 'SDK license notices are missing; refusing to stage a redistributable bundle.'
}
# Ensure the VC runtime is app-local too (required on a clean Windows machine).
$crt = $env:VCToolsRedistDir
if (!$crt -or !(Test-Path "$crt\x64\Microsoft.VC143.CRT")) {
    throw 'Run from an x64 VS 2022 Developer PowerShell (VCToolsRedistDir is required).'
}
Copy-Item "$crt\x64\Microsoft.VC143.CRT\*.dll" $bin -Force
$metadata = @{
    source = 'https://gstreamer.freedesktop.org/'; sdk = (Split-Path $GStreamerRoot -Leaf)
    dlls = @(Get-ChildItem "$bin\*.dll" | ForEach-Object { @{ name = $_.Name; sha256 = (Get-FileHash $_.FullName -Algorithm SHA256).Hash } })
}
$metadata | ConvertTo-Json -Depth 5 | Set-Content "$gst\runtime-manifest.json" -Encoding utf8
Write-Host "Staged Windows runtime at $stage"
