# =============================================================
# ES OPS - Self-signed kod imzalama sertifikasi uretimi
# GELISTIRICI PC'DE TEK SEFERLIK CALISTIRILIR (CI'da degil).
#
# Ciktilari:
#   esops-code-signing.pfx  -> ozel anahtar + sertifika (SIR KAYNAK)
#                              base64'unu GitHub Secret'a ekleyin:
#                              ES_OPS_CODE_SIGNING_PFX
#                              parolayi: ES_OPS_CODE_SIGNING_PASSWORD
#   esops-cert.cer          -> herkese acik sertifika (repoya eklenebilir,
#                              test makinesine kurulur)
#
# OZEL ANAHTAR (pfx) ASLA REPOYA GITMEZ. Yalniz .cer dosyasi repoya
# eklenebilir.
# =============================================================

[CmdletBinding()]
param(
    [string]$OutDir = "",
    [string]$Password = ""
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "..\certs"
}

if ([string]::IsNullOrWhiteSpace($Password)) {
    Write-Host "PFX parolasi girin:"
    $securePass = Read-Host -AsSecureString
} else {
    $securePass = ConvertTo-SecureString $Password -AsPlainText -Force
}

if (-not (Test-Path $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$pfxPath = Join-Path $OutDir "esops-code-signing.pfx"
$cerPath = Join-Path $OutDir "esops-cert.cer"

Write-Host "[ES OPS] Self-signed kod imzalama sertifikasi uretiliyor..."

# RSA (ECC, Smart App Control imza kontrolu tarafindan desteklenmez).
$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject "CN=ES OPS, O=ES, C=TR" `
    -KeyAlgorithm RSA `
    -KeyLength 4096 `
    -KeyUsage DigitalSignature `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddYears(10) `
    -TextExtension @("2.5.29.19={critical}{text}CA=false")

if (-not $cert) {
    Write-Error "Sertifika uretilemedi. Yonetici olarak calistirdiginizdan emin olun."
}

# Pfx olarak disa aktar (ozel anahtar dahil).
Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $securePass | Out-Null
# Herkese acik .cer dosyasi.
Export-Certificate -Cert $cert -FilePath $cerPath -Type CERT | Out-Null

Write-Host "[ES OPS] Sertifika: $($cert.Subject)"
Write-Host "[ES OPS] Thumbprint: $($cert.Thumbprint)"
Write-Host "[ES OPS] Bitis: $($cert.NotAfter.ToShortDateString())"
Write-Host ""
Write-Host "[ES OPS] Olusturuldu:"
Write-Host "    PFX : $pfxPath  (SIR! asla repo'ya koymayin)"
Write-Host "    CER : $cerPath  (public - test makinelerine kurulur)"
Write-Host ""
Write-Host "GitHub Secrets'a ekleyin:"
Write-Host "    ES_OPS_CODE_SIGNING_PFX      = pfx'in base64'u"
Write-Host "    ES_OPS_CODE_SIGNING_PASSWORD = parola"
Write-Host ""
Write-Host "Base64:"
Write-Host "    [Convert]::ToBase64String([IO.File]::ReadAllBytes('$pfxPath'))"