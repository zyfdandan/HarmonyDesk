# Build HarmonyDesk Rust native .so for Mate 80 (aarch64 OHOS)
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$RustDir = Join-Path $RepoRoot "ohos\entry\ohos\rust"
$CoreDir = Join-Path $RepoRoot "ohos\entry\ohos\hdcore"
$LibOut = Join-Path $RepoRoot "ohos\entry\libs\arm64-v8a"
$Target = "aarch64-unknown-linux-ohos"

$mingw = "C:\ProgramData\mingw64\mingw64\bin"
$env:PATH = "$mingw;$env:USERPROFILE\.cargo\bin;" + $env:PATH

if (-not $env:OHOS_NATIVE_HOME) {
  $candidates = @(
    "C:\Users\Administrator\Projects\ohos-sdk\native",
    "C:\Users\Administrator\Projects\ohos-sdk\windows\native",
    "$env:LOCALAPPDATA\Huawei\Sdk\openharmony\native"
  )
  Get-ChildItem "C:\Users\Administrator\Projects\ohos-sdk" -Directory -Recurse -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -eq "native" -and (Test-Path (Join-Path $_.FullName "llvm\bin\clang.exe")) } |
    ForEach-Object { $candidates += $_.FullName }
  foreach ($c in $candidates) {
    if (Test-Path (Join-Path $c "llvm\bin\clang.exe")) {
      $env:OHOS_NATIVE_HOME = $c
      break
    }
  }
}

if (-not $env:OHOS_NATIVE_HOME) {
  throw "OHOS_NATIVE_HOME not set and native clang not found. Extract the OHOS SDK native package first."
}

$env:HARMONYOS_NDK_PATH = $env:OHOS_NATIVE_HOME
$llvm = Join-Path $env:OHOS_NATIVE_HOME "llvm\bin"
$sysroot = Join-Path $env:OHOS_NATIVE_HOME "sysroot"
$env:AR = Join-Path $llvm "llvm-ar.exe"
$env:CC = Join-Path $llvm "clang.exe"
$env:CXX = Join-Path $llvm "clang++.exe"
$env:RANLIB = Join-Path $llvm "llvm-ranlib.exe"
$env:PATH = "$llvm;" + $env:PATH
$env:CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER = Join-Path $llvm "clang.exe"
$env:RUSTFLAGS = "-C link-arg=--target=aarch64-linux-ohos -C link-arg=--sysroot=$sysroot"

Write-Host "OHOS_NATIVE_HOME=$env:OHOS_NATIVE_HOME"
Write-Host "Building $Target ..."

Push-Location $CoreDir
try {
  cargo build --target $Target --release
} finally {
  Pop-Location
}

$so = Join-Path $CoreDir "target\$Target\release\libhdcore.so"
if (-not (Test-Path $so)) {
  throw "Build finished but $so was not produced"
}

New-Item -ItemType Directory -Force -Path $LibOut | Out-Null
Copy-Item $so (Join-Path $LibOut "libhdcore.so") -Force
Write-Host "Copied $(Join-Path $LibOut 'libhdcore.so')"
Get-Item (Join-Path $LibOut "libhdcore.so") | Format-List FullName, Length, LastWriteTime
