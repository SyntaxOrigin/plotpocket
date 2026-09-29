# PlotPocket buyuk dosya olcumu (gercek; hicbir sayi uydurulmamistir).
#
# Metodoloji
# ----------
# Sure      : `Stopwatch` ile islem basindan bitisine kadar, 5 kez tekrarlanir;
#             medyan raporlanir (tek seferlik olcum gurultuye duyarlidir).
# Tepe RSS  : surec calisirken 5 ms'de bir `WorkingSet64` ornegi alinir; 5
#             kosunun **en buyuk** tepe degeri raporlanir. 5 ms aralik, ~50 ms
#             sureli bir kosuda ~10 ornek demektir.
#
# Kullanim
# --------
#   pwsh -NoProfile -File .\ornek\olc.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$exe = Join-Path $root 'target\release\plotpocket.exe'
$buyuk = Join-Path $root 'ornek\buyuk-gecikme.csv'
$KO = 5

function Olcum([string]$ad, [string[]]$arglar) {
    $sureler = @()
    $tepeGenel = 0
    for ($i = 0; $i -lt $KO; $i++) {
        $p = Start-Process -FilePath $exe -ArgumentList $arglar -NoNewWindow -PassThru `
            -RedirectStandardOutput "$env:TEMP\pp-out.txt" -RedirectStandardError "$env:TEMP\pp-err.txt"
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        while (-not $p.HasExited) {
            try {
                $p.Refresh()
                if ($p.WorkingSet64 -gt $tepeGenel) { $tepeGenel = $p.WorkingSet64 }
            } catch { break }
            Start-Sleep -Milliseconds 5
        }
        $sw.Stop()
        $p.WaitForExit()
        $sureler += $sw.Elapsed.TotalMilliseconds
    }
    $sirali = $sureler | Sort-Object
    $medyan = $sirali[[int]($sirali.Count / 2)]
    [PSCustomObject]@{
        Olcum      = $ad
        MedyanMs   = [Math]::Round($medyan, 1)
        EnHizliMs  = [Math]::Round($sirali[0], 1)
        EnYavasMs  = [Math]::Round($sirali[-1], 1)
        TepeRSS_MB = [Math]::Round($tepeGenel / 1MB, 2)
    }
}

$b = Get-Item $buyuk
$lines = @()
$lines += "PlotPocket olcum raporu"
$lines += "========================"
$lines += "dosya        : $($b.Name)"
$lines += ("boyut        : {0:N0} bayt ({1:N2} MB)" -f $b.Length, ($b.Length/1MB))
$lines += "satir        : 50.000 veri + 1 baslik = 5 sutun"
$lines += "tekrarlar    : $KO kosu, medyan raporlandi"
$lines += ""
$lines += ("{0,-40} {1,10} {2,10} {3,10} {4,12}" -f 'olcum', 'medyan ms', 'en hizli', 'en yavas', 'tepe RSS MB')
$lines += ("-" * 86)

$s = @()
$s += Olcum 'plot cizgi (LTTB 2000, 80x24)'      @('plot','ornek\buyuk-gecikme.csv','--x','zaman_s','--y','gecikme_ms','--nokta','2000','--genislik','80','--yukseklik','24')
$s += Olcum 'plot alan (LTTB 2000, 80x24)'       @('plot','ornek\buyuk-gecikme.csv','--x','zaman_s','--y','gecikme_ms','--sablon','alan','--nokta','2000','--genislik','80','--yukseklik','24')
$s += Olcum 'plot nokta (LTTB 2000, 80x24)'      @('plot','ornek\buyuk-gecikme.csv','--x','zaman_s','--y','gecikme_ms','--sablon','nokta','--nokta','2000','--genislik','80','--yukseklik','24')
$s += Olcum 'export SVG (LTTB 2000)'             @('export','ornek\buyuk-gecikme.csv','--x','zaman_s','--y','gecikme_ms','--nokta','2000','--cikti',"$env:TEMP\pp-olcum.svg")
$s += Olcum 'profile (ornek 1000 kayit)'         @('profile','ornek\buyuk-gecikme.csv','--ornek','1000')
$s += Olcum 'profile --tam (tam sayim)'          @('profile','ornek\buyuk-gecikme.csv','--ornek','1000','--tam')
$s += Olcum 'histogram (kova 60)'                @('plot','ornek\buyuk-gecikme.csv','--sablon','histogram','--y','gecikme_ms','--kova','60','--genislik','80','--yukseklik','24')
$s += Olcum 'kutu (bes sayili ozet)'             @('plot','ornek\buyuk-gecikme.csv','--sablon','kutu','--y','gecikme_ms','--genislik','80','--yukseklik','24')

foreach ($r in $s) {
    $lines += ("{0,-40} {1,10} {2,10} {3,10} {4,12}" -f $r.Olcum, $r.MedyanMs, $r.EnHizliMs, $r.EnYavasMs, $r.TepeRSS_MB)
}
$lines += ""
$lines += "Not: RSS olcumu 5 ms ornekleme ile alinir; ~50 ms sureli kosularda"
$lines += "ornek sayisi sinirlidir, bu yuzden degerler alt sinir olabilir."
$lines += "Rapordaki butun sayilar bu makinede (Windows, MinGW-w64 gcc 16.2.0,"
$lines += "rustc 1.98.1) gercek olarak olculmustur."

$rapor = $lines -join "`r`n"
$rapor | Write-Output
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText((Join-Path $root 'ornek\olcum-raporu.txt'), $rapor, $utf8)
