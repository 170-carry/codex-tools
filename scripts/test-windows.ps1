param(
    [string[]]$CargoArgs = @(),
    [string[]]$TestArgs = @()
)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'This test runner requires Windows.' }
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifestPath = Join-Path $repoRoot 'src-tauri/Cargo.toml'
$applicationManifest = Join-Path $repoRoot 'src-tauri/windows-app-manifest.xml'
$loaderPath = Join-Path $repoRoot 'src-tauri/WebView2Loader.dll'

# Tauri links its Windows resources only into application binaries. Rust's
# unit-test harness therefore needs the same Common Controls activation manifest
# before Windows can resolve native dialog imports such as TaskDialogIndirect.
# Update only Cargo's freshly produced test executables, never installed apps.
$metadataJson = & cargo metadata --manifest-path $manifestPath --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$metadata = ($metadataJson -join "`n") | ConvertFrom-Json
$targetRoot = [IO.Path]::GetFullPath($metadata.target_directory).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar

$artifacts = @(& cargo test --manifest-path $manifestPath --lib --no-run --message-format=json @CargoArgs)
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$executables = @($artifacts | ForEach-Object {
    try { $record = $_ | ConvertFrom-Json } catch { return }
    if ($record.reason -eq 'compiler-artifact' -and $record.profile.test -and $record.executable) {
        [IO.Path]::GetFullPath($record.executable)
    }
} | Sort-Object -Unique)
if ($executables.Count -eq 0) { throw 'Cargo produced no library test executable.' }
foreach ($executable in $executables) {
    if (-not $executable.StartsWith($targetRoot, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetExtension($executable) -ne '.exe') {
        throw "Refusing to modify a test artifact outside Cargo's target directory: $executable"
    }
}

if (-not ('CodexToolsTestResources' -as [type])) {
    Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class CodexToolsTestResources {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    private static extern IntPtr BeginUpdateResource(string file, bool deleteExisting);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern bool UpdateResource(IntPtr update, IntPtr type, IntPtr name, ushort language, byte[] data, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern bool EndUpdateResource(IntPtr update, bool discard);
    public static void SetManifest(string file, byte[] data) {
        IntPtr update = BeginUpdateResource(file, false);
        if (update == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
        if (!UpdateResource(update, new IntPtr(24), new IntPtr(1), 1033, data, (uint)data.Length)) {
            int error = Marshal.GetLastWin32Error();
            EndUpdateResource(update, true);
            throw new Win32Exception(error);
        }
        if (!EndUpdateResource(update, false)) throw new Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@
}

$manifestBytes = [IO.File]::ReadAllBytes($applicationManifest)
foreach ($executable in $executables) {
    [CodexToolsTestResources]::SetManifest($executable, $manifestBytes)
    Copy-Item -LiteralPath $loaderPath -Destination (Join-Path (Split-Path -Parent $executable) 'WebView2Loader.dll') -Force
    & $executable @TestArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
