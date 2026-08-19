# =============================================================
# ES OPS - Build sonrasi kod imzalama (CI'da calisir)
# Self-signed sertifika ile MSI + NSIS installer imzalanir.
#
# Gerekli ortam degiskenleri (GitHub Secrets):
#   ES_OPS_CODE_SIGNING_PFX      -> pfx dosyasinin base64 hali
#   ES_OPS_CODE_SIGNING_PASSWORD -> pfx parolasi
#
# Bu betik Tauri build BITTIKTEN sonra calisir; Rust/Tauri koduna
# dokunmaz. Imza, SmartScreen/MotW engelini azaltir; SAC'i asmaz
# (SAC icin prepare-esops-test-machine.ps1 gerekir).
# =============================================================

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$bundleDir = Join-Path $repoRoot "src-tauri\target\release\bundle"

$pfxB64 = $env:ES_OPS_CODE_SIGNING_PFX
$pfxPass = $env:ES_OPS_CODE_SIGNING_PASSWORD

if ([string]::IsNullOrWhiteSpace($pfxB64)) {
    Write-Error "ES_OPS_CODE_SIGNING_PFX ortam degiskeni bos - imzalama yapilamaz."
}

$pfxPath = Join-Path $env:RUNNER_TEMP "esops-signing.pfx"
[System.IO.File]::WriteAllBytes($pfxPath, [Convert]::FromBase64String($pfxB64))

# Sertifikayi ozel anahtarla yukle (okuma icin gecerli sure).
$cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($pfxPath, $pfxPass, "EphemeralKeySet")

if (-not $cert.HasPrivateKey) {
    Write-Error "PFX icinde ozel anahtar yok."
}

# RSA olmasi gerekir (SAC imza kontrolu ECC desteklemez).
if ($cert.PublicKey.Oid.Value -ne "1.2.840.113549.1.1.1") {
    Write-Warning "Sertifika RSA degil - Smart App Control imzayi kabul etmeyebilir."
}

Write-Host "[ES OPS] Sertifika: $($cert.Subject) / Thumbprint: $($cert.Thumbprint)"

# Imzalanacak dosyalar: MSI + NSIS EXE
$files = @()
$msiDir = Join-Path $bundleDir "msi"
if (Test-Path $msiDir) { $files += Get-ChildItem $msiDir -Filter "*.msi" -File }
$nsisDir = Join-Path $bundleDir "nsis"
if (Test-Path $nsisDir) { $files += Get-ChildItem $nsisDir -Filter "*.exe" -File }

if ($files.Count -eq 0) {
    Write-Error "Imzalanacak dosya bulunamadi: $bundleDir"
}

$tsServers = @(
    "http://timestamp.digicert.com",
    "http://timestamp.sectigo.com",
    "http://timestamp.comodoca.com"
)

foreach ($f in $files) {
    Write-Host "[ES OPS] Imzalanıyor: $($f.FullName)"
    $signed = $false
    foreach ($ts in $tsServers) {
        try {
            Set-AuthenticodeSignature -FilePath $f.FullName -Certificate $cert `
                -HashAlgorithm "SHA256" -TimestampServer $ts -ErrorAction Stop | Out-Null
            $signed = $true
            Write-Host "[ES OPS]   OK (timestamp: $ts)"
            break
        } catch {
            Write-Warning "  Timestamp basarisiz ($ts): $($_.Exception.Message)"
        }
    }
    if (-not $signed) {
        try {
            Set-AuthenticodeSignature -FilePath $f.FullName -Certificate $cert `
                -HashAlgorithm "SHA256" -ErrorAction Stop | Out-Null
            Write-Host "[ES OPS]   OK (zaman damgasiz imza)"
        } catch {
            Write-Error "  Imzalama basarisiz: $($f.FullName) -> $($_.Exception.Message)"
        }
    }
}

Write-Host "[ES OPS] Imzalama tamam. $(@($files).Count) dosya imzalandi."

# Sertifika deposunu temizle (CI ortaminda kalici iz birakma).
$cert.Dispose()
Remove-Item $pfxPath -Force -ErrorAction SilentlyContinue