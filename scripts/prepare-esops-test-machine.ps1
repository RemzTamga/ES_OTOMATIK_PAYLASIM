# =============================================================
# ES OPS - Test makinesi hazirlayici
# Yonetici olarak calistirilir. Tek seferlik.
#
# Yaptiklari:
#   1. Smart App Control (SAC) durumunu okur.
#      - Enforcement ise: Microsoft'un belgeli kayit-defteri yontemiyle
#        SAC'i Off konumuna getirir (test amaci, sonra geri acilabilir).
#      - Off ise dokunmaz.
#   2. Self-signed kod imzalama sertifikasini Trusted Root + TrustedPublisher
#      depolarina kurar (installer imzasi guvenilir olur).
#   3. Indirilen installer/MSI dosyalarinda Mark-of-the-Web (MotW) isaretini
#      kaldirir (Unblock-File) -> SmartScreen kontrolu atlanir.
#   4. Istege bagli: installer'i sessiz baslatir.
# =============================================================

[CmdletBinding()]
param(
    [string]$CertPath = "",
    [string]$InstallerPath = "",
    [switch]$AutoInstall,
    [switch]$ForceSacOff
)

$ErrorActionPreference = 'Stop'

function Write-Step {
    param([string]$Msg)
    Write-Host "[ES OPS] $Msg" -ForegroundColor Cyan
}

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-Admin)) {
    Write-Error "Bu betik YONETICI olarak calistirilmalidir."
    exit 1
}

# ---------- 1) Smart App Control durumu ----------
Write-Step "Smart App Control durumu okunuyor..."
$sacMode = "Bilinmiyor"
$citool = "$env:WINDIR\System32\CodeIntegrity\CI.dll"
$citoolExe = "$env:WINDIR\System32\CodeIntegrity\citool.exe"
if (Test-Path $citoolExe) {
    $out = & $citoolExe -lp 2>$null | Out-String
    if ($out -match "VerifiedAndReputableDesktopEvaluation") { $sacMode = "Evaluation" }
    elseif ($out -match "VerifiedAndReputableDesktop") { $sacMode = "Enforcement" }
}
Write-Step "SAC durumu: $sacMode"

$sacReg = "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy"
$sacVal = Get-ItemProperty -Path $sacReg -Name "VerifiedAndReputablePolicyState" -ErrorAction SilentlyContinue
$sacState = if ($sacVal) { $sacVal.VerifiedAndReputablePolicyState } else { $null }

# Enforcement: 1 = On, 0 = Off, 2 = Evaluation
$isOn = ($sacMode -eq "Enforcement") -or ($sacState -eq 1)

if ($isOn) {
    Write-Step "SAC ACIK. OTOMATIK KAPATILIYOR..."
    # Microsoft'un belgeli yontemi (Nisan 2026 KB5083769 sonrasi yonetici/
    # uyarilmis kullanici SAC'i acip kapayabiliyor). Kayit defteri yazimi
    # yalnizca denetim/test amaciyla gecerlidir.
    try {
        Set-ItemProperty -Path $sacReg -Name "VerifiedAndReputablePolicyState" -Value 0 -Type DWord -ErrorAction Stop
        Write-Step "SAC kapatma istegi gonderildi. Windows'u yeniden baslatmak gerekebilir."
    } catch {
        Write-Warning "SAC kayit defteri yazimi basarisiz: $($_.Exception.Message). Yonetici olarak calistirdiginizdan emin olun."
    }
} else {
    Write-Step "SAC kapali veya evaluasyon modunda - dokunulmadi."
}

# ---------- 2) Sertifika kurulumu ----------
if (-not [string]::IsNullOrWhiteSpace($CertPath)) {
    if (-not (Test-Path $CertPath)) {
        Write-Error "Sertifika dosyasi bulunamadi: $CertPath"
        exit 1
    }
    Write-Step "Sertifika kuruluyor: $CertPath"
    try {
        $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($CertPath)
        $storeRoot = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "LocalMachine")
        $storeRoot.Open("ReadWrite")
        $storeRoot.Add($cert)
        $storeRoot.Close()
        $storePub = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPublisher", "LocalMachine")
        $storePub.Open("ReadWrite")
        $storePub.Add($cert)
        $storePub.Close()
        Write-Step "Sertifika Trusted Root + TrustedPublisher depolarina kuruldu."
    } catch {
        Write-Warning "Sertifika kurulumu basarisiz: $($_.Exception.Message)"
    }
} else {
    Write-Step "Sertifika yolu verilmedi - atlandi (CertPath ile verin)."
}

# ---------- 3) Unblock-File (MotW kaldirma) ----------
if (-not [string]::IsNullOrWhiteSpace($InstallerPath)) {
    if (Test-Path $InstallerPath) {
        Write-Step "MotW kaldiriliyor: $InstallerPath"
        try {
            Unblock-File -Path $InstallerPath -ErrorAction Stop
            Write-Step "Unblock-File tamam. (Smart App Control'un notu: Unblock-File SADECE SmartScreen'i atlar; SAC kapaliyken anlamlidir.)"
        } catch {
            Write-Warning "Unblock-File basarisiz: $($_.Exception.Message)"
        }
    } else {
        Write-Warning "Installer dosyasi bulunamadi: $InstallerPath"
    }
} else {
    Write-Step "Installer yolu verilmedi - atlandi (InstallerPath ile verin)."
}

# ---------- 4) Otomatik kurulum ----------
if ($AutoInstall -and -not [string]::IsNullOrWhiteSpace($InstallerPath)) {
    if (Test-Path $InstallerPath) {
        $ext = [System.IO.Path]::GetExtension($InstallerPath).ToLower()
        Write-Step "Kurulum baslatiliyor: $InstallerPath"
        if ($ext -eq ".msi") {
            Start-Process msiexec.exe -ArgumentList "/i", "`"$InstallerPath`"", "/qn", "/norestart" -Wait
        } elseif ($ext -eq ".exe") {
            Start-Process $InstallerPath -ArgumentList "/S" -Wait
        }
        Write-Step "Kurulum islemi tamam."
    }
}

Write-Step "Hazirlik tamam. Sorun varsa CodeIntegrity loglarina bakin (check-esops-blocks.ps1)."