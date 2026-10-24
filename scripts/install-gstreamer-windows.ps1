# Installs the pinned x64 MSVC SDK (runtime + development files).
# Run in an elevated PowerShell on a developer machine; CI Windows runners are elevated.
param([string]$InstallDir = 'C:\gstreamer\1.0\msvc_x86_64')
$ErrorActionPreference = 'Stop'
$version = '1.26.11'
$packages = [ordered]@{
    "gstreamer-1.0-msvc-x86_64-$version.msi" = '31cbc21fa0950b5c1e79c80959b2799805cb05a7a35953a13a9f790776137605'
    "gstreamer-1.0-devel-msvc-x86_64-$version.msi" = 'af0a0720692052aaf5fb0cf2847ef34a87016ef0f914328d516f049e6c86f587'
}
foreach ($name in $packages.Keys) {
    $file = Join-Path $env:TEMP $name
    Invoke-WebRequest "https://gstreamer.freedesktop.org/data/pkg/windows/$version/msvc/$name" -OutFile $file
    if ((Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $packages[$name]) {
        throw "Checksum mismatch: $name"
    }
    $log = "$file.install.log"
    $process = Start-Process msiexec.exe -Wait -PassThru -ArgumentList @('/i', "`"$file`"", '/qn', '/norestart', 'ADDLOCAL=ALL', "INSTALLDIR=`"$InstallDir`"", '/L*v', "`"$log`"")
    if ($process.ExitCode -notin @(0, 3010)) { throw "GStreamer install failed ($($process.ExitCode)); see $log" }
}
if (!(Test-Path "$InstallDir\bin\gst-inspect-1.0.exe")) { throw "SDK not installed at $InstallDir" }
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $InstallDir
$env:PATH = "$InstallDir\bin;$env:PATH"
$env:PKG_CONFIG_PATH = "$InstallDir\lib\pkgconfig"
if ($env:GITHUB_ENV) {
    "GSTREAMER_1_0_ROOT_MSVC_X86_64=$InstallDir" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
    "PKG_CONFIG_PATH=$InstallDir\lib\pkgconfig" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
    "$InstallDir\bin" | Out-File $env:GITHUB_PATH -Append -Encoding utf8
}
& "$InstallDir\bin\gst-inspect-1.0.exe" avdec_aac
if ($LASTEXITCODE) { throw 'GStreamer AAC decoder is missing' }
Write-Host "GStreamer $version x64 MSVC is ready."
