# Export web : compile en wasm, génère le JS de liaison et rassemble le tout dans dist/.
# Les arguments sont passés à cargo, par exemple : .\build-web.ps1 --features autoplay
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
Set-Location $PSScriptRoot

cargo build --profile web --target wasm32-unknown-unknown @args
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$targetDir = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
$wasm = Join-Path $targetDir 'wasm32-unknown-unknown\web\trash-toss.wasm'

if (Test-Path dist) { Remove-Item dist -Recurse -Force }
wasm-bindgen --target web --no-typescript --out-dir dist --out-name trash-toss $wasm
if ($LASTEXITCODE) { exit $LASTEXITCODE }
Copy-Item web\index.html dist\

# Copie compressée, décompressée par index.html : 12 Mo à télécharger au lieu de 60.
$source = [IO.File]::OpenRead((Resolve-Path dist\trash-toss_bg.wasm))
$target = [IO.File]::Create((Join-Path $PSScriptRoot dist\trash-toss_bg.wasm.gz))
$gzip = New-Object IO.Compression.GZipStream($target, [IO.Compression.CompressionLevel]::Optimal)
$source.CopyTo($gzip)
$gzip.Close()
$source.Close()

Get-ChildItem dist -Recurse -File |
    Select-Object @{ n = 'Fichier'; e = { $_.FullName.Substring($PSScriptRoot.Length + 1) } },
                  @{ n = 'Ko'; e = { [int]($_.Length / 1KB) } }
