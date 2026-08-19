# =============================================================
# ES OPS - Engel teshis betigi
# Makinede ES OPS engellendiyse NEDENINI gosterir.
#
#   1. Smart App Control durumu (Off / Evaluation / Enforcement)
#   2. CodeIntegrity olay loglari (3076 evaluasyon, 3077 zorlama)
#      -> hangi dosyanin neden engellendigini gosterir
#   3. Sertifika guven deposu kontrolu (Root + TrustedPublisher)
# =============================================================

[CmdletBinding()]
param(
    [string]$AppPath = ""
)

$ErrorActionPreference = 'Continue'

Write-Host "===== ES OPS Engel Teshisi =====" -ForegroundColor Cyan

# ---------- SAC durumu ----------
Write-Host "`n[1] Smart App Control:" -ForegroundColor Cyan
$citoolExe = "$env:WINDIR\System32\CodeIntegrity\citool.exe"
if (Test-Path $citoolExe) {
    $out = & $citoolExe -lp 2>$null | Out-String
    if ($out -match "VerifiedAndReputableDesktopEvaluation") { Write-Host "    Durum: EVALUATION (engellemez)" -ForegroundColor Yellow }
    elseif ($out -match "VerifiedAndReputableDesktop") { Write-Host "    Durum: ENFORCEMENT (engelleyebilir)" -ForegroundColor Red }
    else { Write-Host "    Durum: OFF / bilinmiyor" -ForegroundColor Green }
} else {
    Write-Host "    citool.exe yok -> SAC yok veya erisilemiyor (Buyuk ihtimal OFF)" -ForegroundColor Green
}

# ---------- CodeIntegrity loglari ----------
Write-Host "`n[2] CodeIntegrity son engelleme kayitlari:" -ForegroundColor Cyan
try {
    $evts = Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-CodeIntegrity/Operational'; Id = 3076, 3077 } -MaxEvents 10 -ErrorAction Stop
    foreach ($e in $evts) {
        $id = $e.Id
        $mod = if ($id -eq 3077) { "ENFORCEMENT-ENGELLEME" } else { "EVALUATION-RAKAM" }
        $file = $e.Properties[0].Value
        $msg = $e.Message -split "`n" | Select-Object -First 1
        Write-Host "    [Event $id / $mod] $file" -ForegroundColor Red
        Write-Host "        $msg" -ForegroundColor Gray
    }
    if (-not $evts) { Write-Host "    Kayit yok (son engelleme bulunamadi)." -ForegroundColor Green }
} catch {
    Write-Host "    CodeIntegrity log okunamadi: $($_.Exception.Message)" -ForegroundColor Gray
}

# ---------- Uygulama dosyasi imza kontrolu ----------
if (-not [string]::IsNullOrWhiteSpace($AppPath)) {
    Write-Host "`n[3] Uygulama imza durumu:" -ForegroundColor Cyan
    if (Test-Path $AppPath) {
        try {
            $sig = Get-AuthenticodeSignature -FilePath $AppPath
            Write-Host "    Durum: $($sig.Status)"
            Write-Host "    Yayimci: $($sig.SignerCertificate.Subject)"
            $root = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "LocalMachine")
            $root.Open("ReadOnly")
            $rootFound = $root.Certificates | Where-Object { $_.Thumbprint -eq $sig.SignerCertificate.Thumbprint }
            $root.Close()
            $pub = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPublisher", "LocalMachine")
            $pub.Open("ReadOnly")
            $pubFound = $pub.Certificates | Where-Object { $_.Thumbprint -eq $sig.SignerCertificate.Thumbprint }
            $pub.Close()
            if ($rootFound) { Write-Host "    Trusted Root: KURULU" -ForegroundColor Green } else { Write-Host "    Trusted Root: YOK" -ForegroundColor Red }
            if ($pubFound) { Write-Host "    TrustedPublisher: KURULU" -ForegroundColor Green } else { Write-Host "    TrustedPublisher: YOK" -ForegroundColor Red }
        } catch {
            Write-Host "    Imza okunamadi: $($_.Exception.Message)" -ForegroundColor Gray
        }
    } else {
        Write-Host "    Dosya yok: $AppPath"
    }
}

# ---------- Sonuc ozeti ----------
Write-Host "`n===== Ozet =====" -ForegroundColor Cyan
Write-Host "SAC ENFORCEMENT ise ve app engelleniyorsa -> tek cozum SAC'i yonetici kapatmak (prepare-esops-test-machine.ps1)."
Write-Host "SAC OFF ise ve SmartScreen engelliyorsa -> Unblock-File + sertifika kurulumu yeterli."