//! PlotPocket / EğriKutu komut satırı arayüzü.
//!
//! Alt komutlar: `plot`, `profile`, `export`, `watch`, `templates`.
//!
//! Çıkış kuralları:
//! - `plot`    → terminal karakter ızgarası (varsayılan) veya `--cikti` verilirse
//!   yalnız dosya yazılır ve stdout'a hiçbir şey basılmaz (boru hattı uyumu).
//! - `profile` → insan okunur tablo veya `--json` ile JSON.
//! - `export` → SVG (veya `svg` biçimi) dosyası.
//! - `watch`   → periyodik yeniden çizim, Ctrl-C'e kadar sürer.
//! - `templates` → şablon listesi.
//!
//! Tüm hatalar `Hata` üzerinden `eprintln!` ile yazılır ve süreç `1` ile çıkar.
//! `main` içinde `unwrap`/`expect`/`panic!` yoktur (`WORKER_CONTRACT.md` § 4.2).

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use plotpocket::ayristirici::AcSecenekleri;
use plotpocket::bicim::Menzil;
use plotpocket::canli::Takip;
use plotpocket::cizim::CizimListesi;
use plotpocket::hata::Sonuc;
use plotpocket::lttb;
use plotpocket::sablon::{Istek, Sablon};
use plotpocket::{profil, svg, terminal};

#[derive(Parser, Debug)]
#[command(
    name = "plotpocket",
    version,
    about = "Buyuk CSV/TSV/JSONL dosyalarindan LTTB indirgemeyle hizli terminal ve SVG grafigi ureten arac.",
    long_about = "PlotPocket (EğriKutu), milyonlarca satırlik CSV/TSV/JSONL dosyalarını \
sabit bellekle okur, LTTB indirgemesiyle ekrana sığan nokta sayısına düşürür ve \
terminalde renksiz karakter ızgarası ya da bağımsız SVG olarak çizer."
)]
struct Cli {
    #[command(subcommand)]
    komut: Komut,
}

#[derive(Subcommand, Debug)]
enum Komut {
    /// Terminalde grafigi cizer (varsayilan gorunum).
    Plot {
        /// Girdi dosyasi (CSV, TSV veya JSONL).
        dosya: PathBuf,
        #[command(flatten)]
        secenek: GrafikSecenekleri,
    },
    /// Sutun tiplerini, bos oranlarini ve araliklari profiller.
    Profile(ProfilSecenekleri),
    /// Bagimsiz SVG dosyasi yazar.
    Export(ExportSecenekleri),
    /// Dosya buyudukce yeni satirlari okuyup yeniden cizer.
    Watch(WatchSecenekleri),
    /// Kullanilabilir sablonlari listeler.
    Templates,
}

#[derive(Args, Debug, Clone)]
struct GrafikSecenekleri {
    /// Grafik sablonu: cizgi, nokta, alan, histogram, kutu.
    #[arg(long, default_value = "cizgi")]
    sablon: String,
    /// x ekseni sutunu (tek sutunlu sablonlarda kullanilmaz).
    #[arg(long)]
    x: Option<String>,
    /// y ekseni sutunu.
    #[arg(long)]
    y: String,
    /// LTTB nokta butcesi (en az 3).
    #[arg(long, default_value_t = lttb::VARSAYILAN_ESIK)]
    nokta: usize,
    /// Alan ayraci; verilmezse dosya adindan veya icerikten bulunur.
    #[arg(long)]
    ayrac: Option<String>,
    /// Ilk satiri baslik sayma.
    #[arg(long)]
    basliksiz: bool,
    /// Terminal sutun sayisi.
    #[arg(long, default_value_t = terminal::VARSAYILAN_GENISLIK)]
    genislik: usize,
    /// Terminal satir sayisi.
    #[arg(long, default_value_t = terminal::VARSAYILAN_YUKSEKLIK)]
    yukseklik: usize,
    /// SVG tuval genisligi.
    #[arg(long, default_value_t = 960.0)]
    tuval_genislik: f64,
    /// SVG tuval yuksekligi.
    #[arg(long, default_value_t = 480.0)]
    tuval_yukseklik: f64,
    /// x ekseni alt siniri.
    #[arg(long)]
    xmin: Option<f64>,
    /// x ekseni ust siniri.
    #[arg(long)]
    xmax: Option<f64>,
    /// y ekseni alt siniri.
    #[arg(long)]
    ymin: Option<f64>,
    /// y ekseni ust siniri.
    #[arg(long)]
    ymax: Option<f64>,
    /// x ekseni birimi (etiket olarak basilir).
    #[arg(long, default_value = "")]
    xbirim: String,
    /// y ekseni birimi (etiket olarak basilir).
    #[arg(long, default_value = "")]
    ybirim: String,
    /// Histogram kova sayisi.
    #[arg(long)]
    kova: Option<usize>,
    /// Ciktiyi dosyaya yaz; stdout'a bir sey basilmaz.
    #[arg(long)]
    cikti: Option<PathBuf>,
    /// SVG yerine terminal cizimi yaz.
    #[arg(long)]
    svg: bool,
}

#[derive(Args, Debug)]
struct ProfilSecenekleri {
    /// Incelenecek dosya.
    dosya: PathBuf,
    /// Orneklenerek bakilacak kayit sayisi (0 = varsayilan 1000).
    #[arg(long, default_value_t = 0)]
    ornek: usize,
    /// Toplam kayit sayisini tam say (dosyayi bastan sona okur).
    #[arg(long)]
    tam: bool,
    /// JSON olarak yaz.
    #[arg(long)]
    json: bool,
    /// Alan ayraci.
    #[arg(long)]
    ayrac: Option<String>,
    /// Ilk satiri baslik sayma.
    #[arg(long)]
    basliksiz: bool,
}

#[derive(Args, Debug)]
struct ExportSecenekleri {
    /// Girdi dosyasi.
    dosya: PathBuf,
    /// Yazilacak SVG dosyasi.
    #[arg(long, default_value = "cikti.svg")]
    cikti: PathBuf,
    #[command(flatten)]
    secenek: GrafikSecenekleri,
}

#[derive(Args, Debug)]
struct WatchSecenekleri {
    /// Izlenecek dosya.
    dosya: PathBuf,
    #[command(flatten)]
    secenek: GrafikSecenekleri,
    /// Yoklama araligi (ms).
    #[arg(long, default_value_t = 500)]
    aralik_ms: u64,
    /// Kuyruk tavani (nokta).
    #[arg(long, default_value_t = plotpocket::canli::VARSAYILAN_TAVAN)]
    tavan: usize,
    /// En fazla bu kadar yeniden ciz (test/otomasyon icin; 0 = sinirsiz).
    #[arg(long, default_value_t = 0)]
    en_fazla_adim: usize,
    /// Her adimda SVG olarak yaz.
    #[arg(long)]
    svg: bool,
    /// Sunucu veya soket yoktur; bu bayrak yalnizca "izle" modunu acik tutar.
    #[arg(long, default_value = "")]
    baslik: String,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // `--help` ve `--version` sıfır kodla çıkar.
            let _ = e.print();
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    ExitCode::SUCCESS
                }
                _ => ExitCode::from(2),
            };
        }
    };
    match calistir(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("plotpocket: {e}");
            ExitCode::FAILURE
        }
    }
}

fn calistir(cli: Cli) -> Sonuc<()> {
    match cli.komut {
        Komut::Plot { dosya, secenek } => {
            let (liste, _istatistik) = grafik_uret(&dosya, &secenek)?;
            // `--svg` aynı çizim listesini SVG olarak yazar: iki çıktı farklı
            // motorlardan değil, aynı modelden beslendiği için aynı görüntüyü
            // üretir.
            if secenek.svg {
                return match secenek.cikti.as_deref() {
                    Some(yol) => svg::svg_yaz(&liste, yol),
                    None => {
                        println!("{}", svg::svg_metni(&liste));
                        Ok(())
                    }
                };
            }
            if let Some(cikti) = secenek.cikti.as_deref() {
                return yaz(
                    cikti,
                    &terminal::metin(&liste, secenek.genislik, secenek.yukseklik)?,
                );
            }
            let metin = terminal::metin(&liste, secenek.genislik, secenek.yukseklik)?;
            println!("{metin}");
            Ok(())
        }
        Komut::Export(ExportSecenekleri {
            dosya,
            secenek,
            cikti,
        }) => {
            let (liste, _istatistik) = grafik_uret(&dosya, &secenek)?;
            svg::svg_yaz(&liste, &cikti)?;
            eprintln!("yazildi: {}", cikti.display());
            Ok(())
        }
        Komut::Profile(p) => {
            let ac = ac_secenekleri(p.ayrac.as_deref(), p.basliksiz)?;
            let mut pr = profil::profille(&p.dosya, &ac, p.ornek)?;
            if p.tam {
                pr.toplam_kayit = Some(profil::toplam_kayit_say(&p.dosya, &ac)?);
            }
            if p.json {
                let metin = serde_json::to_string_pretty(&pr)
                    .unwrap_or_else(|e| format!("{{\"hata\":\"{e}\"}}"));
                println!("{metin}");
            } else {
                print!("{}", profil::metin(&pr));
            }
            Ok(())
        }
        Komut::Watch(w) => izle(&w),
        Komut::Templates => {
            for ad in plotpocket::sablon::SABLON_ADLARI {
                let s = Sablon::adindan(ad)?;
                println!("{:<10} {}", s.ad(), s.aciklama());
            }
            Ok(())
        }
    }
}

/// Grafiği üretir ve istek istatistiklerini döndürür.
fn grafik_uret(dosya: &Path, s: &GrafikSecenekleri) -> Sonuc<(CizimListesi, String)> {
    let sablon = Sablon::adindan(&s.sablon)?;
    let ac = ac_secenekleri(s.ayrac.as_deref(), s.basliksiz)?;
    let mut istek = Istek::yeni(dosya.to_path_buf());
    istek.ac = ac;
    istek.sablon = sablon;
    istek.x_sutun = s.x.clone();
    istek.y_sutun = s.y.clone();
    istek.esik = s.nokta;
    istek.genislik = s.tuval_genislik;
    istek.yukseklik = s.tuval_yukseklik;
    istek.x_araligi = menzil_al(s.xmin, s.xmax);
    istek.y_araligi = menzil_al(s.ymin, s.ymax);
    istek.x_birimi = s.xbirim.clone();
    istek.y_birimi = s.ybirim.clone();
    istek.kova_sayisi = s.kova;
    let g = plotpocket::sablon::uret(&istek)?;
    let ozet = format!(
        "okunan={} indirgenen={} bozuk={} bicak={}",
        g.okunan,
        g.indirgenen,
        g.bozuk,
        sablon.ad()
    );
    Ok((g.cizim, ozet))
}

fn menzil_al(alt: Option<f64>, ust: Option<f64>) -> Option<Menzil> {
    match (alt, ust) {
        (Some(a), Some(b)) => Some(Menzil::yeni(a, b)),
        (None, Some(b)) => Some(Menzil::yeni(f64::NEG_INFINITY, b)),
        (Some(a), None) => Some(Menzil::yeni(a, f64::INFINITY)),
        (None, None) => None,
    }
}

fn ac_secenekleri(ayrac: Option<&str>, basliksiz: bool) -> Sonuc<AcSecenekleri> {
    let bayt = match ayrac {
        None => None,
        Some(a) => {
            let b = a.as_bytes();
            if b.len() != 1 {
                return Err(plotpocket::hata::Hata::GecersizAyrac {
                    deger: a.to_string(),
                });
            }
            Some(b[0])
        }
    };
    Ok(AcSecenekleri {
        ayrac: bayt,
        baslik: !basliksiz,
    })
}

fn yaz(yol: &Path, icerik: &str) -> Sonuc<()> {
    use std::io::Write as _;
    let mut f =
        std::fs::File::create(yol).map_err(|kaynak| plotpocket::hata::Hata::CiktiHatasi {
            yol: yol.to_path_buf(),
            kaynak,
        })?;
    f.write_all(icerik.as_bytes())
        .map_err(|kaynak| plotpocket::hata::Hata::CiktiHatasi {
            yol: yol.to_path_buf(),
            kaynak,
        })?;
    Ok(())
}

/// Canlı takip döngüsü.
fn izle(w: &WatchSecenekleri) -> Sonuc<()> {
    let s = &w.secenek;
    let sablon = Sablon::adindan(&s.sablon)?;
    let ac = ac_secenekleri(s.ayrac.as_deref(), s.basliksiz)?;
    let mut takip = Takip::baslat(&w.dosya, ac, s.x.clone(), &s.y, w.tavan)?;
    let aralik = std::time::Duration::from_millis(w.aralik_ms.max(1));
    let mut adim = 0usize;
    loop {
        let cizim = kuyruktan_cizim(&takip, s, sablon, &w.dosya)?;
        if w.svg {
            match &s.cikti {
                Some(cikti) => svg::svg_yaz(&cizim, cikti)?,
                None => println!("{}", svg::svg_metni(&cizim)),
            }
        } else {
            let m = terminal::metin(&cizim, s.genislik, s.yukseklik)?;
            // Ekranı temizle: ANSI kaçış dizileri standart çıktıda kullanılır.
            print!("\x1b[2J\x1b[H{m}");
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
        }
        if w.en_fazla_adim > 0 && adim + 1 >= w.en_fazla_adim {
            break;
        }
        adim += 1;
        std::thread::sleep(aralik);
        takip.yokla()?;
    }
    eprintln!(
        "bitti: {} nokta, {} yeni satir, {} kirpildi, {} donme",
        takip.nokta_sayisi(),
        takip.toplam_yeni,
        takip.kirpildi,
        takip.donme
    );
    Ok(())
}

/// Kuyruktan tek seferlik çizim listesi üretir.
fn kuyruktan_cizim(
    takip: &Takip,
    s: &GrafikSecenekleri,
    sablon: Sablon,
    dosya: &std::path::Path,
) -> Sonuc<CizimListesi> {
    let nokta_butcesi = if s.nokta == 0 {
        lttb::VARSAYILAN_ESIK
    } else {
        s.nokta
    };
    let xs: Vec<f64> = takip.kuyruk().iter().map(|p| p.0).collect();
    let ys: Vec<f64> = takip.kuyruk().iter().map(|p| p.1).collect();
    let r = lttb::indir(&xs, &ys, nokta_butcesi)?;
    let (nx, ny): (Vec<f64>, Vec<f64>) = r.noktalar.iter().map(|(x, y)| (*x, *y)).unzip();

    let mut liste = CizimListesi::yeni(s.tuval_genislik, s.tuval_yukseklik);
    let dosya_adi = dosya
        .file_name()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    liste.baslik = match s.x.as_deref() {
        Some(x) => format!("{dosya_adi}  {} = f({x})", s.y),
        None => format!("{dosya_adi}  {}", s.y),
    };
    liste.alt_bilgi = format!(
        "canli: {} nokta | {} yeni satir | {} kirpildi | {} donme",
        takip.kuyruk().len(),
        takip.toplam_yeni,
        takip.kirpildi,
        takip.donme
    );
    let (ax, ay) = liste.alan_baslangic();
    let (bx, by) = liste.alan_bitis();
    let xm = Menzil::degerlerden(&nx).kisitla(menzil_al(s.xmin, s.xmax));
    let ym = Menzil::degerlerden(&ny).kisitla(menzil_al(s.ymin, s.ymax));
    let xo = plotpocket::bicim::Olcek::yeni(xm, ax, bx);
    let yo = plotpocket::bicim::Olcek::yeni(ym, by, ay);
    let noktalar: Vec<(f64, f64)> = nx
        .iter()
        .zip(ny.iter())
        .filter_map(|(x, y)| Some((xo.ekrana(*x)?, yo.ekrana(*y)?)))
        .collect();
    use plotpocket::cizim::{Oge, Renk};
    if !noktalar.is_empty() {
        match sablon {
            Sablon::Nokta => liste.ekle(Oge::Nokta {
                noktalar,
                yaricap: 2.2,
                renk: Renk::Bir,
            }),
            Sablon::Alan => {
                let taban = yo.ekrana(yo.menzil().alt).unwrap_or(liste.yukseklik);
                liste.ekle(Oge::Alan {
                    noktalar,
                    taban,
                    saydamlik: 0.18,
                    renk: Renk::Bir,
                    kalinlik: 1.2,
                });
            }
            _ => liste.ekle(Oge::Cizgi {
                noktalar,
                renk: Renk::Bir,
                kalinlik: 1.6,
            }),
        }
    }
    plotpocket::cizim::eksen_koy(&mut liste, xo, yo, &s.xbirim, &s.ybirim)?;
    Ok(liste)
}
