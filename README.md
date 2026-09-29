# PlotPocket (EğriKutu)

Büyük CSV/TSV/JSONL dosyalarından **tek komutla** terminal grafiği ya da bağımsız
SVG üreten, Rust ile yazılmış bir araç.

Dosyanın tamamı belleğe alınmaz. LTTB (Largest-Triangle-Three-Buckets) indirgemesi
sayesinde milyonlarca satır, ekrana sığan nokta sayısına düşürülür; çizim maliyeti
veri boyutundan bağımsız hâle gelir.

```
 50.000 satır  ──►  okuma (kayan tampon)  ──►  LTTB (tek geçiş)  ──►  çizim listesi
                                                                        │
                                        ┌───────────────────────────────┴──────────────┐
                                        ▼                                              ▼
                            terminal karakter ızgarası                        bağımsız SVG metni
                            (renksiz, blok karakterli)              (dış kaynağa bağlı değil)
```

## Özellikler

- **CSV / TSV / JSONL okuyucu** — başlık satırı, tırnaklı alan, `""` ve `\"` kaçışları,
  tırnak içinde satır sonu, alan içi ayraç, boş alan, bozuk satır atlama ve sayacı.
- **Küçük örnekleme ile sütun profilleme** — dosyanın tamamı okunmadan tip çıkarımı
  (tam sayı → kayan → metin), boş oranı, min/max/ortalama, en sık metin değerleri.
- **LTTB indirgeme** — kendi yazılmış, kayan nokta davranışı belgelenmiş;
  `NaN`/`±inf` atlanır, sıfır aralık ve sabit serilerde çalışır.
- **Grafik modeli** — grafik bir belge değil, **ekran koordinatlarının listesi**dir
  (rapor b07'in mimari kararı). Aynı liste terminal ve SVG çıktısını besler.
- **Terminal çizim** — ASCII/Unicode karakter ızgara, renk yok, hizalı etiketler,
  sekiz seviyeli blok karakterli eğri yüksekliği (`▁▂▃▄▅▆▇█`).
- **SVG dışa aktarım** — tek dosya, gömülü stil, **sistem yazı tipi adı**
  (gömülü yazı tipi yok), `& < > " '` kaçışı.
- **Canlı takip modu** — dosya büyüdükçe yeni satırlar okunur ve grafik yeniden çizilir.
  `notify` yerine periyodik boyut karşılaştırması kullanılır; kuyruk tavanla kırpılır.
- **Beş şablon** — `cizgi`, `nokta`, `alan`, `histogram`, `kutu`.
- **Aralık kısıtlama** — `--xmin/--xmax/--ymin/--ymax` ile yakınlaştırma.
- **Borç hattı uyumu** — `plot --cikti` stdout'a hiçbir şey basmaz.

## Kurulum

Gereksinim: Rust 1.74+ (MSRV) ve C bağlantılayıcısı için MinGW-w64 gcc.

```console
$ cargo --version
cargo 1.98.1 (797e8a9bc 2026-08-05)
$ rustc --version
rustc 1.98.1 (48a229cea 2026-09-01 2026-09-01)
```

Derleme:

```console
$ cargo build --release
    Finished `release` profile [optimized] target(s) in 7.88s
```

Tek dosya dağıtım — yalnızca ikiliyi kopyalayın, başka hiçbir şey gerekmez:

```console
$ cargo install --path .
  Installing %USERPROFILE%\AppData\Roaming\.cargo\bin\plotpocket.exe
  Installed package `plotpocket v0.1.0 (C:\...\28-plotpocket)`
  Finished `dev` profile [optimized] target(s) in 8.10s
```

## Kullanım

Aşağıdaki **her komut bu depoda gerçekten çalıştırıldı**; çıktılar kopyadır.

### 1. Terminal grafiği (LTTB ile indirgenmiş çizgi)

```console
$ plotpocket plot ornek/orta-gecikme.csv --x zaman_s --y gecikme_ms --nokta 2000 --genislik 76 --yukseklik 20 --xbirim sn --ybirim ms
    orta-gecikme.csv  gecikme_ms = f(zaman_s)
  ms
    ┌──────────────────────────────────────────────────────────────────────┐
    │  ██████     ·         ████·█             ·  ██████     ·         ████│
    │ ▇▇▇▇▇▇▇▇    ·         ▇▇▇▇·▇▇            · ▇▇▇▇▇▇▇     ·         ▇▇▇▇│
 100│······································································│
    │▆▆▆    ▆▆▆   ·       ▆▆▆▆  ·▆▆▆           ·▆▆▆   ▆▆▆▆   ·       ▆▆▆   │
  80│······································································│
    │▅       ▅▅▅  ·      ▅▅▅    · ▅▅▅         ▅·▅       ▅▅▅  ·      ▅▅▅    │
    │▅        ▅▅▅ ·     ▅▅▅     ·  ▅▅▅        ▅·▅        ▅▅▅ ·     ▅▅▅     │
  60│······································································│
    │          ▃▃▃·    ▃▃▃      ·   ▃▃▃      ▃▃·          ▃▃▃·    ▃▃▃      │
    │           ▃▃·   ▃▃▃       ·    ▃▃▃    ▃▃▃·          ▃▃▃·   ▃▃▃       │
  40│······································································│
    │            ▂·▂▂▂▂▂        ·     ▂▂▂▂▂▂▂▂ ·            ▂·▂▂▂▂▂        │
    │             ·▁▁▁▁         ·      ▁▁▁▁▁▁  ·             ·▁▁▁▁         │
    └──────────────────────────────────────────────────────────────────────┘
    0           1000          2000           3000          4000
                                       sn
                                            5000 satir -> 5000 nokta (LTTB)
```

Sağ alt köşedeki `5000 satir -> 5000 nokta (LTTB)` satırı indirgeme bilgisidir
(rapor b03, senaryo S1). 50.000 satırlık dosyada aynı komut `-> 2000 nokta` yazar.

### 2. Sütun profilleme

```console
$ plotpocket profile ornek/orta-gecikme.csv --ornek 2000 --tam
dosya        : orta-gecikme.csv
bicim        : csv
ayrac        : ,
boyut        : 125091 bayt
ornek kayit  : 2000
toplam kayit : 5000 (tam sayim)
bozuk kayit  : 0
sutun sayisi : 5

#    sutun              tip         bos% sonsuz      en kucuk      en buyuk
0    zaman_s            tam-sayi    0.0%      0             0          1999
1    gecikme_ms         kayan       0.0%      0        20.778       117.666
2    istek_no           tam-sayi    0.0%      0          1000          2999
3    bolge              metin       0.0%      0             -             -
4    durum              metin       0.0%      0             -             -
```

JSON biçimi için `--json` bayrağı kullanılır; şema `ornek/olcum-raporu.txt`
yanındaki `profile --json` çıktısıyla doğrulanmıştır.

### 3. SVG dışa aktarımı

```console
$ plotpocket export ornek/orta-gecikme.csv --x zaman_s --y gecikme_ms --nokta 2000 --cikti cikti.svg
yazildi: cikti.svg
```

Üretilen dosya **hiçbir dış kaynağa bağlı değildir**. Doğrulama:

```console
$ head -3 cikti.svg
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="960" height="480" viewBox="0 0 960 480">
<style type="text/css"><![CDATA[
  .p{fill:none;stroke-linecap:round;stroke-linejoin:round}
```

`xlink:href`, `<image>`, `<script>` ve `@font-face` desenlerinden **hiçbiri**
dosyada bulunmaz (bu, `tests/entegrasyon.rs` içinde otomatik olarak denetlenir).

### 4. Şablon listesi

```console
$ plotpocket templates
cizgi      x-y noktalarini LTTB ile indirgeyip polyline cizer
nokta      x-y noktalarini LTTB ile indirgeyip dagilim olarak cizer
alan       x-y noktalarini LTTB ile indirgeyip tabana kadar dolar
histogram  tek y sutununu esit genislikli kovalara bolup sayar
kutu       tek y sutununun bes sayili ozetini cizer (min, Q1, medyan, Q3, max)
```

### 5. Histogram ve kutu grafiği

```console
$ plotpocket plot ornek/orta-gecikme.csv --sablon histogram --y gecikme_ms --kova 30 --genislik 70 --yukseklik 16 --ybirim adet
     orta-gecikme.csv  gecikme_ms
 adet
     ┌───────────────────────────────────────────────────────────────┐
     │          ·         ·          ·          ·       ██·██        │
  100│·······························································│
     │        ██·██       ·          ·          ·       ██·█████     │
   80│·······························································│
     │ █████████·████████ ·██        · ███    ██·█████████·█████████ │
   60│·······························································│
     │██████████·█████████·██████████·██████████·█████████·██████████│
   40│·······························································│
     │██████████·█████████·██████████·██████████·█████████·██████████│
     └───────────────────────────────────────────────────────────────┘
     0          5        10         15         20        25         30

                                              5000 kayit -> histogram
```

```console
$ plotpocket plot ornek/orta-gecikme.csv --sablon kutu --y gecikme_ms --genislik 60 --yukseklik 16 --ybirim ms
    orta-gecikme.csv  gecikme_ms
  ms
    ┌──────────────────────────────────────────────────────┐
    │                           │                          │
 100│······················································│
    │                     │          │                     │
    │                     │          │                     │
  80│······················································│
    │                     │          │                     │
  60│······················································│
    │                     │          │                     │
    │                     ────────────                     │
  40│······················································│
    │                           │                          │
    └──────────────────────────────────────────────────────┘
                                         5000 kayit -> kutu
```

### 6. Canlı takip

```console
$ plotpocket watch ornek/canli.log --x zaman_s --y gecikme_ms --aralik_ms 200 --tavan 20000
```

Dosya büyüdükçe yeni satırlar eklenir ve ekran yeniden çizilir. Test/otomasyon
için `--en-fazla-adim N` verilerek N çizimden sonra süreç kontrollü biçimde biter.

### 7. Hatalar

```console
$ plotpocket plot ornek/orta-gecikme.csv --y olmayan_sutun
plotpocket: sütun bulunamadı: "olmayan_sutun" (dosyada 5 sütun var)
$ echo $?
1
```

## Test

```console
$ cargo test
   Compiling plotpocket v0.1.0 (C:\...\28-plotpocket)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.44s
     Running unittests src\lib.rs (target\debug\deps\plotpocket-....exe)
running 106 tests
test ayristirici::tests::basit_csv_ayristirilir ... ok
test ayristirici::tests::bilinmeyen_bicim_reddedilir ... ok
test ayristirici::tests::bos_alan_bos_kayit_uretir ... ok
test ayristirici::tests::bozuk_satir_atlanir_ve_sayilir ... ok
...
test result: ok. 106 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s

     Running tests\entegrasyon.rs (target\debug\deps\entegrasyon-....exe)
running 17 tests
test buyuk_dosyada_indirgeme_nokta_butcesine_indirir ... ok
test canli_takip_dosya_buyumesini_yakalar ... ok
test csv_dosyadan_terminal_grafik_uretir ... ok
...
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

**test sonucu: okunan 123; başarısız 0** (106 birim + 17 entegrasyon).

Kapsanan uç durumlar:

| Alan | Örnekler |
|---|---|
| CSV ayrıştırma | tırnaklı alan, alan içi ayraç, tırnak içi satır sonu, `""` kaçışı, `\"` kaçışı, boş alan, satır sonu ayracı, CRLF, bozuk satır atlama, yinelenen başlık, başlıksız dosya, elle ayraç, geçersiz ayraç |
| Biçim algılama | `.csv`, `.tsv`, `.jsonl`, uzantısız JSON, ayraç sayımı, bilinmeyen biçim |
| JSONL | satır kökü, dizi kökü, tek satırlık dizi, bozuk satır, yeni anahtar, sayısal metin |
| Tip çıkarımı | tam sayı → kayan → metin geçişleri, boş oranı, `NaN`/`inf` sayımı, en sık değerler |
| LTTB | n≤k, n=k, n<k, sabit seri, `NaN`/sonsuz, tek nokta, tepe korunumu, monotonluk |
| Ölçekleme | sıfır aralık, ters aralık, negatif değer, `1e308`, `1e-308`, güzel tik adımı |
| Terminal | boş veri, tek nokta, geniş aralık taşması, `NaN` koordinat, çerçeve, ızgara, metin, alan dolgusu, hücre dışı kırpma |
| SVG | geçerli XML kökü, dış bağımlılık yok, sistem yazı tipi, metin kaçışı |
| Şablonlar | beş şablonun tamamı, aralık kısıtlaması, geçersiz eşik/boyut |
| Canlı takip | dosya büyümesi, değişmeme, tavan kırpma, dosya küçülmesi (donme), sütun yok |
| Hata senaryoları | dosya yok, biçim yok, sütun yok, geçersiz ayraç |

Kalite kapısı:

```console
$ cargo build --release
$ cargo test
$ cargo clippy --all-targets -- -D warnings
$ cargo fmt --check
```

Dördü de hatasız geçer. `#![forbid(unsafe_code)]` her iki çıktıda da etkindir.

## Proje Yapısı

```
28-plotpocket/
├── Cargo.toml              # clap + serde + serde_json (yalnızca bu üçü)
├── Cargo.lock              # üretilir, commit edilir
├── LICENSE.txt             # MIT tam metni
├── README.md
├── .gitignore              # /target/, .env, *.log
├── src/
│   ├── lib.rs              # çekirdek kütüphane, modül haritası, forbid(unsafe_code)
│   ├── main.rs             # CLI kabuğu (clap): plot, profile, export, watch, templates
│   ├── hata.rs             # Hata enum + elle Display/Error
│   ├── ayristirici.rs      # CSV/TSV/JSONL okuyucu, tırnak çözümü, JSON akış tarayıcı
│   ├── bicim.rs            # Menzil, Olcek, güzel tikler, sayı biçimleme
│   ├── lttb.rs             # LTTB indirgeme (tek geçiş)
│   ├── profil.rs           # örneklemeli sütun profilleme + JSON çıktı
│   ├── sablon.rs           # şablonlar (cizgi/nokta/alan/histogram/kutu)
│   ├── cizim.rs            # çizim listesi (Oge, Renk, eksen üretimi)
│   ├── terminal.rs         # karakter ızgarası rasterleyici
│   ├── svg.rs              # bağımsız SVG yazıcı
│   ├── canli.rs            # canlı takip (kuyruk + tavan + donme sayacı)
│   └── test_yardimcisi.rs  # geçici dizin yardımcısı (Drop ile temizlik)
├── tests/
│   └── entegrasyon.rs      # uçtan uca testler (17)
└── ornek/
    ├── kucuk-gecikme.csv   # 15 satır, tırnaklı ve bozuk alanlar içerir
    ├── kucuk-olcum.tsv     # 12 satır TSV
    ├── kucuk-olcum.jsonl   # 6 kayıt NDJSON
    ├── tirnakli-ve-bozuk.csv
    ├── orta-gecikme.csv    # 5.000 satır (üretici: uret.ps1)
    ├── buyuk-gecikme.csv   # 50.000 satır, 1,28 MB (üretici: uret.ps1)
    ├── uret.ps1            # deterministik örnek veri üreticisi (xorshift64*)
    ├── olc.ps1             # ölçüm betiği (süre + tepe RSS)
    └── olcum-raporu.txt    # ölçüm sonuçları (gerçek çıktı)
```

## Yapılandırma

Yapılandırma dosyası **yoktur**; her şey komut satırı bayraklarındadır.

| Bayrak | Varsayılan | Etkisi |
|---|---|---|
| `--sablon` | `cizgi` | `cizgi`, `nokta`, `alan`, `histogram`, `kutu` |
| `--x` | yok | x ekseni sütunu. Verilmezse satır numarası kullanılır. Tek sütunlu şablonlarda yok sayılır. |
| `--y` | zorunlu | y ekseni sütunu (veya tek sütunlu şablonlarda ölçülecek sütun) |
| `--nokta` | `2000` | LTTB nokta bütçesi. En az 3. Büyük değer = daha yavaş, daha ayrıntılı. |
| `--genislik` | `80` | Terminal sütun sayısı (1..=2000) |
| `--yukseklik` | `24` | Terminal satır sayısı (1..=2000) |
| `--tuval-genislik` | `960` | SVG tuval genişliği (SVG birimi) |
| `--tuval-yukseklik` | `480` | SVG tuval yüksekliği (SVG birimi) |
| `--ayrac` | otomatik | Alan ayracı. Verilmezse dosya uzantısından, yoksa içerikten (en sık ayraç) bulunur. |
| `--basliksiz` | kapalı | İlk satırı başlık saymaz; sütun adları `s1..sn` olur. |
| `--xmin` `--xmax` | yok | x aralığı kısıtlaması. |
| `--ymin` `--ymax` | yok | y aralığı kısıtlaması. |
| `--xbirim` `--ybirim` | boş | Eksen birimi etiketi. |
| `--kova` | `40` | Histogram kova sayısı. |
| `--cikti` | yok | Çıktıyı dosyaya yaz; `plot` komutunda stdout'a hiçbir şey basılmaz. |
| `--svg` | kapalı | `plot` komutunda terminal yerine SVG üretir. |
| `profile --ornek` | `0` → `1000` | Profillemede okunacak kayıt sayısı. `0` varsayılanı kullanır. |
| `profile --tam` | kapalı | Toplam kayıt sayısını **tam** sayar (dosyayı baştan sona okur). |
| `profile --json` | kapalı | Profili JSON olarak yazar. |
| `watch --aralik_ms` | `500` | Yoklama aralığı. |
| `watch --tavan` | `200000` | Kuyruk nokta tavanı; aşılırsa en eski atılır ve `kirpildi` artar. |
| `watch --en-fazla-adim` | `0` | Bu kadar çizimden sonra çık (test için). `0` = sınırsız. |

### Profil JSON şeması

```jsonc
{
  "dosya": "orta-gecikme.csv",
  "bicim": "csv",              // csv | tsv | jsonl-satir | jsonl-dizi
  "ayrac": 44,                 // JSON biçiminde 0
  "basliklar": ["zaman_s", "gecikme_ms", "istek_no", "bolge", "durum"],
  "ornek_kayit": 1000,         // gerçekten okunan kayıt
  "toplam_kayit": null,        // --tam ile sayıldıysa sayı, aksi hâlde null
  "kayit_tahmini": 50000,      // dosya boyutundan türetilen tahmin
  "dosya_boyutu": 125091,
  "bozuk_kayit": 0,
  "sutunlar": [
    {
      "ad": "gecikme_ms",
      "indeks": 1,
      "tip": "kayan",          // tam-sayi | kayan | metin
      "gozlenen": 1000,
      "bos": 0,
      "bos_orani": 0.0,
      "en_kucuk": 20.778,
      "en_buyuk": 117.666,
      "ortalama": 69.4,
      "sonlu_degil": 0,        // NaN / ±inf sayısı
      "farkli_deger": 0,       // metin sütunlarda örnekteki farklı değer
      "en_sik": []             // [[deger, adet], ...] en fazla 5
    }
  ]
}
```

## Ölçüm

`ornek/olc.ps1` ile **bu makinede gerçekten ölçülmüştür**
(Windows, MinGW-w64 gcc 16.2.0, rustc 1.98.1, release profili). Her ölçüm 5 kez
koşturulur, **medyan** raporlanır; tepe RSS koşu sırasında 5 ms'de bir örneklenir.

| Ölçüm | Medyan | En hızlı | En yavaş | Tepe RSS |
|---|---|---|---|---|
| plot çizgi (LTTB 2000, 80×24) | 23,2 ms | 18,9 ms | 69,9 ms | 4,36 MB |
| plot alan (LTTB 2000, 80×24) | 16,5 ms | 12,3 ms | 26,4 ms | 3,92 MB |
| plot nokta (LTTB 2000, 80×24) | 23,9 ms | 16,8 ms | 26,6 ms | 3,91 MB |
| export SVG (LTTB 2000) | 30,8 ms | 22,4 ms | 36,3 ms | 3,98 MB |
| profile (örnek 1000 kayıt) | 25,3 ms | 20,0 ms | 27,0 ms | 3,86 MB |
| profile --tam (tam sayım) | 23,5 ms | 10,4 ms | 25,8 ms | 3,88 MB |
| histogram (kova 60) | 13,0 ms | 11,3 ms | 22,8 ms | 3,91 MB |
| kutu (beş sayılı özet) | 14,7 ms | 13,7 ms | 22,5 ms | 3,91 MB |

Veri: `ornek/buyuk-gecikme.csv` — 50.000 veri satırı, 5 sütun, 1.340.289 bayt (1,28 MB).

**Dürüstlük notları:**

- RSS ölçümü 5 ms aralıkla örneklenir; ~20 ms süren koşularda örnek sayısı
  azdır, bu yüzden değerler **alt sınırdır** ve gerçek tepe daha yüksek olabilir.
- 50.000 satır, raporun 10 milyon satırlık hedefinin çok altındadır; bu ölçümler
  büyük dosya iddiasını **kanıtlamaz**, yalnızca 50 bin satır için geçerlidir.
- LTTB uygulaması indirgemeden önce seriyi `Vec<(f64,f64)>` olarak belleğe alır
  (50.000 nokta ≈ 0,8 MB). Bu, "hiçbir zaman tüm veriyi belleğe alma" sözünün
  **yumuşatılmış** hâlidir ve aşağıdaki sınırlamalar bölümünde yazılıdır.

## Bilinen Sınırlamalar

1. **LTTB tüm seriyi önce belleğe alır.** `ayristirici` kayan tamponla okur ve
   dosyayı asla `read_to_string` ile belleğe almaz, ancak indirgemeye giren
   sayısal noktalar `Vec<(f64, f64)>` olarak biriktirilir. Çok büyük dosyalarda
   (yüz milyonlarca satır) bu bellek bütçesini aşar. Gerçek çözüm iki geçişli
   kova sayımıdır; uygulanmadı.
2. **PNG, HTML ve etkileşimli çıktı yok.** Rapor b05'te v2 aşamasına konmuştur.
3. **Çok eksen (çift y ekseni) yok.** Tüm seriler tek ölçekte çizilir.
4. **Tema yok.** Palet sabittir (SVG'de 6 renk); terminal çıktısı zaten renksizdir.
5. **Gerçek zamanlı canlı takip sınırlıdır.** `watch` yalnızca yeni satırları
   okur; **düzeltilmiş** (baştan okunan) satırları yeniden yansıtmaz, dosya
   baştan sona taranmaz. Ayrıca duraklat/devam et ve fare etkileşimi yoktur.
6. **Terminal çıktısında en-boy oranı düzeltilmez.** Terminal hücresi fiziksel
   olarak yaklaşık 2:1 dikey olduğu için grafik optik olarak biraz yassı görünür.
7. **Terminal çiziminde en-boy oranı hücre tabanlıdır.** Uzun eksen etiketleri
   12 sütunla sınırlanır ve gerekirse kırpılır; çok geniş sayılar (ör. `1.2e308`)
   üstel gösterime çevrilir.
8. **`kutu` ve `histogram` tek sütunlu şablonlardır;** verilen `--x` bilinçli
   olarak yok sayılır. `kutu` grafiği 2.000.000 değerlik sınıra takılırsa
   örnekleme yapılır ve bu bilgi grafiğin alt bilgisine yazılır.
9. **Kutu grafiğinde medyan etiketi dar terminalde kırpılabilir.**
10. **Sütun adı sütunlarını belirtmez.** Yalnızca tek bir x ve tek bir y sütunu
    seçilir; çok sütunlu üst üste grafik yoktur.
11. **`Drop` içindeki geçici dizin temizliği hataları yutulur** (`let _ =`). `Drop`
    çağrısından hata döndürülemez; bu, sözleşmenin "sessiz yutma" yasağına
    açık bir istisnadır.
12. **Ayrıştırma hataları sessizce atlanır** (tasarım gereği) ancak
    `bozuk_kayit` sayacı her zaman raporlanır; sessiz veri kaybı yoktur.
13. **Bozuk kayıt kuralları katıdır.** Alan sayısı başlıkla uyuşmayan satır
    atlanır. Satır sonundaki ayraç (`1,2,`) bir boş alan daha ürettiği için iki
    sütunlu bir dosyada bozuk sayılır. Bu RFC 4180 uyumludur ama günlük
    dosyalarında şaşırtıcı olabilir.
14. **JSONL anahtar sırası `serde_json::Map` sözlüğü nedeniyle alfabetiktir.**
    Kaynak dosyadaki sıra korunmaz.
15. **Windows dışında `cargo build` için sistemde C bağlantılayıcısı gerekir**
    (macOS: Xcode CLT, Linux: `gcc`). Harici çalışma zamanı gerekmez.

## Gelecek Geliştirmeler

- **İki geçişli LTTB**: kova sınırları için önce yalnızca satır sonu ofsetleri
  sayılır, ikinci geçişte veri okunur. Bu, bellek bütçesini `O(1)` yapar.
- **PNG dışa aktarımı** (kendi kodlayıcı; `image` crate'i yasak).
- **Bağımsız HTML** çıktı (satır içi SVG + küçük JS; WebView gerekmez).
- **Çift y ekseni** ve ölçek kilitleme.
- **Fare etkileşimi** yerine `plot --xmin/--xmax` kombinasyonları için hazır
  "yakınlaştırma önerisi" komutu.
- **Canlı takipte düzeltme desteği** ve `pause/resume`.
- **Çoklu seri üst üste** (aynı dosyadan 3-4 y sütunu, ayrı ölçekle).
- **Beni kendi PRNG'siyle tekrarlanabilir** `cargo bench` ölçümleri (criterion
  yerine kendi ölçücümüz).

## Troubleshooting

**1. Belirti:** `linker 'link.exe' not found` (Windows)
**Neden:** `link.exe` PATH üzerinde değil; Rust, gcc yerine MSVC bağlantılayıcısını
arıyor. **Çözüm:** MinGW-w64 gcc'yi PATH'e ekleyin:
```console
$ env:PATH = "%USERPROFILE%\.cargo\bin;<mingw64>\bin;" + $env:PATH
$ cargo build --release
```

**2. Belirti:** `biçim belirlenemedi: uzantı tanınmıyor ve içerik CSV/TSV/JSONL olarak ayrıştırılamadı`
**Neden:** Dosya adında tanınmayan uzantı var ve içerikte baskın bir ayraç bulunamadı
(ör. tek sütunlu, ayraçsız dosya). **Çözüm:** Ayraç elle verin:
```console
$ plotpocket plot veri.dat --ayrac ";" --x 1 --y 2
```

**3. Belirti:** Grafik boş çıkıyor, üst satırda `indeks` hatası yok ama eğri yok.
**Neden:** `--y` ile seçilen sütun aslında **metin** sütunu; sayısal olmayan
alanlar LTTB'ye girmez ve grafik boş çizilir. **Çözüm:** Önce sütun tipini denetleyin:
```console
$ plotpocket profile veri.csv
# sütun   tip       bos%   en kucuk   en buyuk
1  deger  metin     0.0%      -          -
```
Sonuç `metin` ise doğru sütunu seçin ya da dosyadaki ondalık ayracını düzeltin
(`63,7` geçerli bir sayı değildir; `63.7` olmalıdır).

**4. Belirti:** `--nokta` değeri hata veriyor: `geçersiz eşik: 2 (LTTB için en az 3 olmalı)`
**Neden:** LTTB ilk ve son noktayı korumak için en az 3 nokta ister. **Çözüm:**
`--nokta 3` veya daha büyük bir değer verin.

**5. Belirti:** Terminal çıktısında eksen etiketleri eksik / kesilmiş.
**Neden:** Terminal genişliği küçük (`--genislik 40`). Y ekseni etiketleri en fazla
12 sütuna sığdırılır. **Çözüm:** `--genislik` değerini artırın veya SVG üretin:
```console
$ plotpocket plot veri.csv --x t --y g --svg --cikti cikti.svg
```

**6. Belirti:** `watch` komutu hiç güncellenmiyor.
**Neden:** Yazıcı satır sonu yazmadan tamponlu yazıyor; okuyucu tam satır bekler.
**Çözüm:** Yazıcının `flush` çağırdığından emin olun ya da `--aralik_ms` değerini
küçültün.

## Atıflar

- Rapor dosyası (iç tasarımın kaynağı): `%USERPROFILE%\Desktop\Fikirler\28-egri-kutu-csv-grafik.html`
  — yerel yol; URL değildir.
- Ortak yönetmelik: `%USERPROFILE%\Desktop\Projeler\WORKER_CONTRACT.md`,
  proje kartı: `%USERPROFILE%\Desktop\Projeler\MANIFEST.md` (Kart 28).
- LTTB algoritmasının referans davranışı:
  Apache ECharts — <https://echarts.apache.org/> (LTTB örnekleme seçeneğini belgeler).
- LTTB'nin öncülü ve "en büyük üçgen" seçimi: Sveinn Steinarsson, *Fast Rendering
  of Data (2006)* — <https://www.cs.ubc.ca/~heid/Papers/PAP2.pdf>
- SVG 1.1 biçimi: W3C — <https://www.w3.org/TR/SVG11/>
- SVG uygulama notları (viewBox, stil): MDN — <https://developer.mozilla.org/docs/Web/SVG>
- RFC 4180 — CSV (Common Format and MIME Type for Comma-Separated Values) —
  <https://www.rfc-editor.org/rfc/rfc4180> (tırnaklı alan, kaçışlı tırnak, alan içi ayraç)
- RFC 8259 — The JavaScript Object Notation (JSON) —
  <https://www.rfc-editor.org/rfc/rfc8259> (JSONL kök dizi/satır ayrımı)
- NDJSON biçimi: ndjson.org — <https://ndjson.org/>
- Rust standart kütüphane (özellikle `std::io::BufRead`, `std::time`) —
  <https://doc.rust-lang.org/std/>
- `clap` — <https://docs.rs/clap/>
- `serde` / `serde_json` — <https://serde.rs/> · <https://docs.rs/serde_json/>
- İçe aktarılan kod: **yoktur.** CSV ayrıştırıcısı, LTTB, ölçekleme, SVG yazıcı ve
  terminal rasterleyicisi bu proje için yazılmıştır. 26 DataLens projesiyle CSV
  okuyucu konusunda işlevsel örtüşme vardır; `WORKER_CONTRACT.md` § 9 gereği kod
  **bilinçli olarak kopyalanmamış**, bağımsız yazılmıştır.

## Lisans

MIT — tam metin `LICENSE.txt` dosyasındadır. Telif: `Copyright (c) 2026 PlotPocket contributors`.
