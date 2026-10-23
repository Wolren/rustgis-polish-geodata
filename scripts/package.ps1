# Package dane-pl as a .rgplugin and write the registry entry.
#
#   .\scripts\package.ps1
#
# Produces:
#   dist\dane-pl\                    the plugin folder (load it while developing)
#   dist\dane-pl-<version>.rgplugin  the package to install or publish
#   dist\dane-pl.json                the entry a registry submission needs

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$manifest = Get-Content (Join-Path $root 'plugin.json') -Raw | ConvertFrom-Json
$id = $manifest.id
$version = $manifest.version
$dist = Join-Path $root 'dist'
$folder = Join-Path $dist $id

Write-Host "building $id $version"
Push-Location $root
try { cargo build --release } finally { Pop-Location }

if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
New-Item -ItemType Directory -Force -Path (Join-Path $folder 'bin') | Out-Null
Copy-Item (Join-Path $root 'plugin.json') $folder
Copy-Item (Join-Path $root 'target\release\dane-pl.exe') (Join-Path $folder 'bin')

$package = Join-Path $dist "$id-$version.rgplugin"
$zip = Join-Path $dist "$id-$version.zip"
foreach ($f in @($package, $zip)) { if (Test-Path $f) { Remove-Item $f -Force } }
Compress-Archive -Path (Join-Path $folder '*') -DestinationPath $zip -CompressionLevel Optimal
Move-Item $zip $package -Force

$hash = (Get-FileHash $package -Algorithm SHA256).Hash.ToLower()
$size = (Get-Item $package).Length

$entry = [ordered]@{
    id                 = $id
    name               = $manifest.name
    version            = $version
    description        = $manifest.description
    author             = $manifest.author
    license            = $manifest.license
    runtime            = 'process'
    permissions        = $manifest.permissions
    minRustgisVersion  = $manifest.minRustgisVersion
    package            = "https://example.com/$id-$version.rgplugin"
    sha256             = $hash
    size               = $size
}
$entry | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $dist "$id.json") -Encoding UTF8

Write-Host ("package: {0} ({1:N0} bytes, sha256 {2})" -f $package, $size, $hash)
Write-Host ("folder:  {0}" -f $folder)
