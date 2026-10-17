[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$dist = Join-Path $repository 'dist'

Push-Location $repository
try {
    $metadataJson = cargo metadata --no-deps --format-version 1 --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $metadata = $metadataJson | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object { $_.name -eq 'codex-titlebar-meter' }).version
    if (-not $version) { throw 'Package version was not found in Cargo metadata' }
    $package = Join-Path $dist "codex-titlebar-meter-v$version-windows-x64"

    cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo test failed' }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo build --release failed' }

    $archive = "$package.zip"
    if ((Test-Path -LiteralPath $package) -or (Test-Path -LiteralPath $archive)) {
        throw "Release output already exists: $package"
    }
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $metadata.target_directory 'release\codex-titlebar-meter.exe') -Destination (Join-Path $package 'codex-titlebar-meter.exe')
    Copy-Item -LiteralPath (Join-Path $repository 'install.ps1') -Destination $package
    Copy-Item -LiteralPath (Join-Path $repository 'uninstall.ps1') -Destination $package
    Copy-Item -LiteralPath (Join-Path $repository 'README.md') -Destination $package
    Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination $package

    Compress-Archive -Path (Join-Path $package '*') -DestinationPath $archive -CompressionLevel Optimal
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$archive.sha256" -Value "$hash  $(Split-Path -Leaf $archive)" -Encoding ascii
    Write-Host "Created $archive" -ForegroundColor Green
}
finally {
    Pop-Location
}
