# SPDX-License-Identifier: GPL-3.0-only
#
# Build the unsigned Windows installer (WiX v5 MSI + portable zip).
#
# Usage: packaging/package-windows.ps1 -Arch x64|arm64

param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("x64", "arm64")]
  [string]$Arch
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$CargoToml = Get-Content -Raw (Join-Path $Root "Cargo.toml")
$Version = $null
if ($CargoToml -match '(?m)^version = "(.+?)"') {
  $Version = $Matches[1]
}
$Triple = if ($Arch -eq "arm64") {
  "aarch64-pc-windows-msvc"
} else {
  "x86_64-pc-windows-msvc"
}
$Out = Join-Path $Root "target\package"
$Stage = Join-Path $Out "stage"
$Cards = Join-Path $Stage "cards"

function Fail([string]$Message) {
  Write-Error $Message
  exit 1
}

if (-not $Version) { Fail "cannot read the version from Cargo.toml" }

Set-Location $Root

if (Test-Path $Out) {
  Remove-Item $Out -Recurse -Force
}
New-Item -ItemType Directory -Path $Cards -Force | Out-Null

cargo build --release --target $Triple -p rivercrossing --locked
if ($LASTEXITCODE -ne 0) { Fail "cargo build failed" }

Copy-Item "target\$Triple\release\rivercrossing.exe" $Stage
if (-not (Test-Path (Join-Path $Stage "rivercrossing.exe"))) {
  Fail "binary not found after build"
}

Get-ChildItem -Path "design\assets\cards" -Filter "*-2x.png" |
  Copy-Item -Destination $Cards
Copy-Item "packaging\branding\rivercrossing.ico" $Stage
Copy-Item "LICENSE" $Stage
Copy-Item "NOTICE" $Stage

# WiX v5 has no component-local <Files> glob, so generate one component per
# card bitmap and link the fragment into the main .wxs.
$components = @()
foreach ($f in (Get-ChildItem -Path $Cards -Filter "*.png" | Sort-Object Name)) {
  $id = ($f.BaseName -replace '[^A-Za-z0-9]', '_')
  $components += "      <Component Id=`"Card_$id`" Guid=`"*`" Directory=`"CardsFolder`">"
  $components += "        <File Id=`"File_$id`" Source=`"$($f.FullName)`" />"
  $components += "      </Component>"
}
$cardsWxs = Join-Path $Out "cards.wxs"
$cardsXml = @"
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Directory Id="CardsFolder" Name="cards" />
    </DirectoryRef>
    <ComponentGroup Id="CardComponents">
$($components -join "`n")
    </ComponentGroup>
  </Fragment>
</Wix>
"@
Set-Content -Path $cardsWxs -Value $cardsXml -Encoding utf8NoBOM

# WiX v5 is the pinned toolchain (architecture.md §10); 5.0.2 is the latest v5.
# The unversioned `wix` tool now resolves to v7, which demands the OSMF EULA.
dotnet tool install --global wix --version 5.0.2
if ($LASTEXITCODE -ne 0) { Fail "wix tool install failed" }

$Wxs = Join-Path $Root "packaging\wix\rivercrossing.wxs"
$Msi = Join-Path $Out "RiverCrossing-$Version-windows-$Arch.msi"
wix build -arch $Arch -d "SourceDir=$Stage" -d "AppVersion=$Version" `
  -o $Msi $Wxs $cardsWxs
if ($LASTEXITCODE -ne 0) { Fail "wix build failed" }

$Zip = Join-Path $Out "RiverCrossing-$Version-windows-$Arch-portable.zip"
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip

Write-Host "Built $Msi and $Zip"
