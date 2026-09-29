#!/usr/bin/env pwsh
# PlotPocket ornek veri ureteci (deterministik, rastgelelik crate'i yok).
#
# Kullanim:
#   pwsh -NoProfile -File .\ornek\uret.ps1
#
# Uretilen dosyalar:
#   ornek\orta-gecikme.csv     5.000 satir
#   ornek\buyuk-gecikme.csv   50.000 satir
#
# Deterministik xorshift tabanli PRNG kullanilir; ayni tohum ayni dosyayi
# uretir, boylece README'deki olcumler tekrarlanabilir.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

# --- xorshift64* PRNG -------------------------------------------------------
$script:State = [uint64]88172645463325252
function Next-Random {
    $x = $script:State
    $x = $x -bxor ($x -shl 13)
    $x = $x -bxor ($x -shr 7)
    $x = $x -bxor ($x -shl 17)
    $script:State = $x
    # [0,1) araligina indirger.
    return ($x -shr 11) / [double]9007199254740992
}

function New-OrnekYol([string]$ad, [int]$satir) {
    $yol = Join-Path $root $ad
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append("zaman_s,gecikme_ms,istek_no,bolge,durum`n")
    $bolgeler = @('tr-1','eu-1','us-1','ap-1')
    for ($i = 0; $i -lt $satir; $i++) {
        $t = $i * 0.05
        # Ustel gurultu + gunluk dalga + ani tepe (LTTB'nin korumasi gerekir).
        $gurultu = (Next-Random - 0.5) * 18.0
        $gunluk = 40.0 * [Math]::Sin($i / 240.0)
        $tepe = 0.0
        if ($i % 9973 -eq 0 -and $i -gt 0) { $tepe = 260.0 }
        $deger = 60.0 + $gunluk + $gurultu + $tepe
        if ($deger -lt 1.0) { $deger = 1.0 }
        $bolge = $bolgeler[$i % 4]
        $durum = if ($deger -gt 240.0) { 'hata' } else { 'ok' }
        # Nokta ayraci her makinede `.` kalsin diye InvariantCulture kullanilir
        # (Turkce locale'de `F3` bicim tanimlayicisi hata verir).
        $sayi = $deger.ToString('F3', [System.Globalization.CultureInfo]::InvariantCulture)
        [void]$sb.Append("$i,$sayi,$((1000 + $i)),$bolge,$durum")
        [void]$sb.Append("`n")
    }
    [System.IO.File]::WriteAllText($yol, $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))
    $kb = [Math]::Round((Get-Item $yol).Length / 1KB, 1)
    Write-Output ("{0}: {1} satir, {2} KB" -f $ad, $satir, $kb)
}

New-OrnekYol 'orta-gecikme.csv' 5000
New-OrnekYol 'buyuk-gecikme.csv' 50000
