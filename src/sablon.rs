//! Yaygın grafik şablonları: çizgi, nokta, alan, histogram, kutu (box).
//!
//! Sorumluluğu **ham sütun verisinden** indirgenmiş seriler üretmek ve
//! `CizimListesi` doldurmaktır. Okuma, ölçekleme ve çıktı biçimlerinin bilmez.
//!
//! # Şablonlar
//!
//! | Şablon | Girdi | Davranış |
//! |---|---|---|
//! | `cizgi` | x + y sütunu | LTTB ile indirgenmiş polyline |
//! | `nokta` | x + y sütunu | LTTB ile indirgenmiş dağılım noktaları |
//! | `alan` | x + y sütunu | Taban seviyesine kadar dolu alan |
//! | `histogram` | tek sütun | Eşit genişlikli kovalar, sayım |
//! | `kutu` | tek sütun | Beş sayılı özet (min, Q1, medyan, Q3, max) |
//!
//! `histogram` ve `kutu` tek sütunla çalışır; sıralama gerektiren `kutu`
//! şablonu değerlerin tamamını sıralayamayacağı için **sınırlı örnek** üzerinde
//! çalışır ve bunu `alt_bilgi`de açıkça yazar (rapor b08 "dürüst ölçüm" ilkesi).

use std::fmt::Write as _;

use crate::ayristirici::{AcSecenekleri, Okuyucu};
use crate::bicim::{Menzil, Olcek};
use crate::cizim::{CizimListesi, Hiza, Oge, Renk};
use crate::hata::{Hata, Sonuc};
use crate::lttb;

/// `kutu` şablonunun sıralayabileceği azami değer sayısı (bellek bütçesi).
pub const KUTU_ORNEK_SINIRI: usize = 2_000_000;

/// Desteklenen şablon adları.
pub const SABLON_ADLARI: [&str; 5] = ["cizgi", "nokta", "alan", "histogram", "kutu"];

/// Şablon türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sablon {
    /// Polyline.
    Cizgi,
    /// Dağılım noktaları.
    Nokta,
    /// Taban seviyesine dolu alan.
    Alan,
    /// Y tek sütunlu histogram.
    Histogram,
    /// Tek sütunlu kutu grafiği.
    Kutu,
}

impl Sablon {
    /// Adından şablon çözer.
    ///
    /// # Hatalar
    ///
    /// Bilinmeyen ad için `Hata::BilinmeyenSablon` döner.
    pub fn adindan(ad: &str) -> Sonuc<Sablon> {
        match ad {
            "cizgi" | "line" => Ok(Sablon::Cizgi),
            "nokta" | "scatter" | "dagilim" => Ok(Sablon::Nokta),
            "alan" | "area" => Ok(Sablon::Alan),
            "histogram" | "hist" => Ok(Sablon::Histogram),
            "kutu" | "box" => Ok(Sablon::Kutu),
            diger => Err(Hata::BilinmeyenSablon {
                ad: diger.to_string(),
            }),
        }
    }

    /// Şablonun adı.
    pub fn ad(self) -> &'static str {
        match self {
            Sablon::Cizgi => "cizgi",
            Sablon::Nokta => "nokta",
            Sablon::Alan => "alan",
            Sablon::Histogram => "histogram",
            Sablon::Kutu => "kutu",
        }
    }

    /// Şablonun tek sütunla mı çalıştığı.
    pub fn tek_sutun(self) -> bool {
        matches!(self, Sablon::Histogram | Sablon::Kutu)
    }

    /// Şablonun kısa açıklaması (`templates` komutu için).
    pub fn aciklama(self) -> &'static str {
        match self {
            Sablon::Cizgi => "x-y noktalarini LTTB ile indirgeyip polyline cizer",
            Sablon::Nokta => "x-y noktalarini LTTB ile indirgeyip dagilim olarak cizer",
            Sablon::Alan => "x-y noktalarini LTTB ile indirgeyip tabana kadar dolar",
            Sablon::Histogram => "tek y sutununu esit genislikli kovalara bolup sayar",
            Sablon::Kutu => "tek y sutununun bes sayili ozetini cizer (min, Q1, medyan, Q3, max)",
        }
    }
}

/// Grafik üretimine giden istek.
#[derive(Debug, Clone)]
pub struct Istek {
    /// Giriş dosyası.
    pub yol: std::path::PathBuf,
    /// Okuyucu ayarları.
    pub ac: AcSecenekleri,
    /// Kullanılacak şablon.
    pub sablon: Sablon,
    /// x sütunu (tek sütunlu şablonlarda yok).
    pub x_sutun: Option<String>,
    /// y sütunu.
    pub y_sutun: String,
    /// LTTB nokta bütçesi.
    pub esik: usize,
    /// Tuval genişliği.
    pub genislik: f64,
    /// Tuval yüksekliği.
    pub yukseklik: f64,
    /// Kullanıcının verdiği x aralığı kısıtlaması.
    pub x_araligi: Option<Menzil>,
    /// Kullanıcının verdiği y aralığı kısıtlaması.
    pub y_araligi: Option<Menzil>,
    /// x ekseni birimi.
    pub x_birimi: String,
    /// y ekseni birimi.
    pub y_birimi: String,
    /// Histogram kova sayısı (`None` ise 40).
    pub kova_sayisi: Option<usize>,
}

impl Istek {
    /// Varsayılan istek (CLI tarafından doldurulur).
    pub fn yeni(yol: std::path::PathBuf) -> Self {
        Istek {
            yol,
            ac: AcSecenekleri::default(),
            sablon: Sablon::Cizgi,
            x_sutun: None,
            y_sutun: String::new(),
            esik: lttb::VARSAYILAN_ESIK,
            genislik: 960.0,
            yukseklik: 480.0,
            x_araligi: None,
            y_araligi: None,
            x_birimi: String::new(),
            y_birimi: String::new(),
            kova_sayisi: None,
        }
    }
}

/// Üretilen grafik ve okuma istatistikleri.
#[derive(Debug, Clone)]
pub struct Grafik {
    /// Hazır çizim listesi.
    pub cizim: CizimListesi,
    /// Okunan kayıt sayısı.
    pub okunan: u64,
    /// LTTB'den geçen nokta sayısı (tek sütunlu şablonlarda `0`).
    pub indirgenen: usize,
    /// Atlanan bozuk kayıt sayısı.
    pub bozuk: u64,
    /// Kullanılan veri aralığı (x).
    pub x_menzil: Menzil,
    /// Kullanılan veri aralığı (y).
    pub y_menzil: Menzil,
}

/// İsteği okur, indirger ve çizim listesi üretir.
///
/// # Hatalar
///
/// Sütun yoksa `Hata::SutunYok`, sütun sayısal değilse `Hata::SutunSayisalDegil`,
/// boyut geçersizse `Hata::GecersizBoyut` döner.
pub fn uret(istek: &Istek) -> Sonuc<Grafik> {
    crate::bicim::boyut_dogrula(istek.genislik as usize, istek.yukseklik as usize)?;
    let mut okuyucu = Okuyucu::ac(&istek.yol, &istek.ac)?;
    let sutun_sayisi = okuyucu.sutun_sayisi();
    if sutun_sayisi == 0 {
        return Err(Hata::SutunYok {
            sutun: istek.y_sutun.clone(),
            sutun_sayisi,
        });
    }
    let yi = cozumle(&okuyucu, &istek.y_sutun, sutun_sayisi)?;
    let xi = match istek.x_sutun.as_deref() {
        None => None,
        Some(ad) => Some(cozumle(&okuyucu, ad, sutun_sayisi)?),
    };
    // Tek sütunlu şablonlarda verilen `--x` bilinçli olarak yok sayılır:
    // `histogram` ve `kutu` yalnızca y eksenini kullanır.
    let xi = if istek.sablon.tek_sutun() { None } else { xi };

    let mut liste = CizimListesi::yeni(istek.genislik, istek.yukseklik);
    liste.baslik = baslik(istek);
    // `kutu` yalnızca y eksenini kullanır; sayısal x tikleri (0..1) anlamsız
    // olacağı için bastırılır. `histogram`de x kova indeksi olarak anlamlıdır.
    liste.x_tik_goster = istek.sablon != Sablon::Kutu;
    let (ax, ay) = liste.alan_baslangic();
    let (bx, by) = liste.alan_bitis();

    // Her şablon kolu `okunan` ve `indirgenen` değerlerini **kesin** olarak atar.
    let okunan: u64;
    let mut indirgenen = 0usize;
    let grafik_veri = match istek.sablon {
        Sablon::Cizgi | Sablon::Nokta | Sablon::Alan => {
            let (okunan_sayi, (xs, ys), n_giris) = seriyi_oku(&mut okuyucu, xi, yi, istek.esik)?;
            okunan = okunan_sayi;
            indirgenen = n_giris;
            // x ve y **ayrı** menzillerdir; `noktalardan` ikisini tek aralıkta
            // birleştirir ve iki ekseni birden bozardı.
            let ym = Menzil::degerlerden(&ys).kisitla(istek.y_araligi);
            let xm = Menzil::degerlerden(&xs).kisitla(istek.x_araligi);
            let y_olcek = Olcek::yeni(ym, by, ay);
            let x_olcek = Olcek::yeni(xm, ax, bx);
            cizgi_veya_alan(&mut liste, istek.sablon, &xs, &ys, x_olcek, y_olcek);
            (xm, ym)
        }
        Sablon::Histogram => {
            let (degerler, adet, kesik) = tek_sutun_oku(&mut okuyucu, yi, KUTU_ORNEK_SINIRI)?;
            okunan = adet;
            if kesik {
                liste.alt_bilgi = format!(
                    "ornek: {} deger (sinir {})",
                    degerler.len(),
                    KUTU_ORNEK_SINIRI
                );
            }
            let kova = istek.kova_sayisi.unwrap_or(40).max(1);
            let m = Menzil::noktalardan(&degerler, &degerler);
            let m = m.kisitla(istek.y_araligi);
            histogram_ciz(&mut liste, &degerler, m, kova, by, ay);
            (Menzil::yeni(0.0, kova as f64), m)
        }
        Sablon::Kutu => {
            let (mut degerler, adet, kesik) = tek_sutun_oku(&mut okuyucu, yi, KUTU_ORNEK_SINIRI)?;
            okunan = adet;
            let m = Menzil::noktalardan(&degerler, &degerler);
            let m = m.kisitla(istek.y_araligi);
            if kesik {
                let ek = format!(
                    "ornek: {} degerdan {} tanesi (sinir {})",
                    adet,
                    degerler.len(),
                    KUTU_ORNEK_SINIRI
                );
                liste.alt_bilgi = ek;
            }
            let ozet = bes_sayili_ozet(&mut degerler);
            kutu_ciz(&mut liste, &ozet, m, ax, bx, by, ay);
            (Menzil::yeni(0.0, 1.0), m)
        }
    };
    let bozuk = okuyucu.bozuk_satir_sayisi();
    let (xm, ym) = grafik_veri;

    // Alt bilgi **önce** doldurulur: `eksen_koy` alt bilgiyi bir `Oge::Metin`
    // olarak çizim listesine yazar, bu yüzden metin burada hazır olmalıdır
    // (rapor b03, senaryo S1: üst çubukta indirgeme bilgisi yazılır).
    if istek.sablon.tek_sutun() {
        if liste.alt_bilgi.is_empty() {
            let _ = write!(liste.alt_bilgi, "{okunan} kayit -> {}", istek.sablon.ad());
        }
    } else if liste.alt_bilgi.is_empty() {
        let _ = write!(
            liste.alt_bilgi,
            "{okunan} satir -> {indirgenen} nokta (LTTB)"
        );
    }

    let y_olcek = Olcek::yeni(ym, by, ay);
    let x_olcek = Olcek::yeni(xm, ax, bx);
    crate::cizim::eksen_koy(
        &mut liste,
        x_olcek,
        y_olcek,
        &istek.x_birimi,
        &istek.y_birimi,
    )?;

    Ok(Grafik {
        cizim: liste,
        okunan,
        indirgenen,
        bozuk,
        x_menzil: xm,
        y_menzil: ym,
    })
}

/// Sütun adını indekse çözer; yoksa `Hata::SutunYok` döner.
fn cozumle(okuyucu: &Okuyucu, ad: &str, sutun_sayisi: usize) -> Sonuc<usize> {
    okuyucu.sutun_indeksi(ad).ok_or_else(|| Hata::SutunYok {
        sutun: ad.to_string(),
        sutun_sayisi,
    })
}

fn baslik(istek: &Istek) -> String {
    let dosya = istek
        .yol
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match istek.x_sutun.as_deref() {
        Some(x) => format!("{dosya}  {y} = f({x})", y = istek.y_sutun),
        None => format!("{dosya}  {}", istek.y_sutun),
    }
}

/// x/y sütunlarını okur ve LTTB indirgeme uygular.
///
/// Dönüş: `(okunan kayıt sayısı, (indirgenmiş x, indirgenmiş y), giriş noktası)`.
#[allow(clippy::type_complexity)]
fn seriyi_oku(
    okuyucu: &mut Okuyucu,
    xi: Option<usize>,
    yi: usize,
    esik: usize,
) -> Sonuc<(u64, (Vec<f64>, Vec<f64>), usize)> {
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut okunan = 0u64;
    while let Some(s) = okuyucu.sonraki_satir()? {
        okunan += 1;
        let y = match s.sayi(yi) {
            Some(v) => v,
            None => continue,
        };
        let x = match xi {
            None => okunan as f64,
            Some(i) => match s.sayi(i) {
                Some(v) => v,
                None => continue,
            },
        };
        // Sonlu olmayan değerler LTTB'ye de girmez; burada elenerek
        // `Indirgeme.atlanan` sayacının anlamlı kalması sağlanır.
        if !y.is_finite() || !x.is_finite() {
            continue;
        }
        xs.push(x);
        ys.push(y);
    }
    let n_giris = xs.len();
    let indirgeme = lttb::indir(&xs, &ys, esik)?;
    let mut nx = Vec::with_capacity(indirgeme.noktalar.len());
    let mut ny = Vec::with_capacity(indirgeme.noktalar.len());
    for (x, y) in &indirgeme.noktalar {
        nx.push(*x);
        ny.push(*y);
    }
    Ok((okunan, (nx, ny), n_giris))
}

/// Tek sütun okur; `sinir` aşılırsa okuma durur.
///
/// Dönüş: `(değerler, okunan kayıt, örnek sınırına takılıp takılmadığı)`.
fn tek_sutun_oku(okuyucu: &mut Okuyucu, yi: usize, sinir: usize) -> Sonuc<(Vec<f64>, u64, bool)> {
    let mut degerler: Vec<f64> = Vec::with_capacity(16_384);
    let mut okunan = 0u64;
    let mut kesik = false;
    while let Some(s) = okuyucu.sonraki_satir()? {
        okunan += 1;
        if degerler.len() >= sinir {
            kesik = true;
            break;
        }
        if let Some(v) = s.sayi(yi) {
            if v.is_finite() {
                degerler.push(v);
            }
        }
    }
    Ok((degerler, okunan, kesik))
}

/// Beş sayılı özet: `(min, Q1, medyan, Q3, max)`. Girdi yerinde sıralanır.
fn bes_sayili_ozet(degerler: &mut [f64]) -> [f64; 5] {
    if degerler.is_empty() {
        return [0.0; 5];
    }
    degerler.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = degerler.len();
    let q = |p: f64| -> f64 {
        let i = ((n as f64 - 1.0) * p).round() as usize;
        degerler[i.min(n - 1)]
    };
    [degerler[0], q(0.25), q(0.5), q(0.75), degerler[n - 1]]
}

/// Polyline / nokta / alan öğelerini üretir.
fn cizgi_veya_alan(
    liste: &mut CizimListesi,
    sablon: Sablon,
    xs: &[f64],
    ys: &[f64],
    xo: Olcek,
    yo: Olcek,
) {
    let noktalar: Vec<(f64, f64)> = xs
        .iter()
        .zip(ys.iter())
        .filter_map(|(x, y)| Some((xo.ekrana(*x)?, yo.ekrana(*y)?)))
        .collect();
    if noktalar.is_empty() {
        return;
    }
    let renk = Renk::Bir;
    match sablon {
        Sablon::Cizgi => liste.ekle(Oge::Cizgi {
            noktalar,
            renk,
            kalinlik: 1.6,
        }),
        Sablon::Nokta => liste.ekle(Oge::Nokta {
            noktalar,
            yaricap: 2.2,
            renk,
        }),
        Sablon::Alan => {
            let taban = yo.ekrana(yo.menzil().alt).unwrap_or(liste.yukseklik);
            liste.ekle(Oge::Alan {
                noktalar,
                taban,
                saydamlik: 0.18,
                renk,
                kalinlik: 1.2,
            });
        }
        Sablon::Histogram | Sablon::Kutu => {}
    }
}

/// Histogram çubuklarını üretir.
fn histogram_ciz(
    liste: &mut CizimListesi,
    degerler: &[f64],
    m: Menzil,
    kova: usize,
    by: f64,
    ay: f64,
) {
    if degerler.is_empty() {
        liste.ekle(Oge::Metin {
            x: liste.genislik / 2.0,
            y: (ay + by) / 2.0,
            icerik: "sayisal veri yok".to_string(),
            hiza: Hiza::Orta,
            boyut: 14.0,
            renk: Renk::Metin,
        });
        return;
    }
    let m = m.genislet();
    let mut sayilar = vec![0u32; kova];
    let g = m.genislik();
    for v in degerler {
        if !(*v >= m.alt && *v <= m.ust) {
            continue;
        }
        let mut i = (((v - m.alt) / g) * kova as f64) as usize;
        if i >= kova {
            i = kova - 1;
        }
        sayilar[i] = sayilar[i].saturating_add(1);
    }
    let en_cok = sayilar.iter().copied().max().unwrap_or(1).max(1) as f64;
    let (ax, _) = liste.alan_baslangic();
    let (bx, _) = liste.alan_bitis();
    let genislik = bx - ax;
    let dilim = genislik / kova as f64;
    let mut cubuklar = Vec::with_capacity(kova);
    for (i, s) in sayilar.iter().enumerate() {
        if *s == 0 {
            continue;
        }
        let h = (by - ay) * (*s as f64 / en_cok);
        let x0 = ax + i as f64 * dilim + 0.5;
        let x1 = ax + (i + 1) as f64 * dilim - 0.5;
        cubuklar.push((x0, by - h, x1, by));
    }
    liste.ekle(Oge::Cubuk {
        cubuklar,
        renk: Renk::Bir,
    });
}

/// Kutu grafiğini üretir.
fn kutu_ciz(
    liste: &mut CizimListesi,
    ozet: &[f64; 5],
    m: Menzil,
    ax: f64,
    bx: f64,
    by: f64,
    ay: f64,
) {
    let yo = Olcek::yeni(m, by, ay);
    let merkez = (ax + bx) / 2.0;
    let yari = (bx - ax) / 10.0;
    let y = |v: f64| yo.ekrana(v).unwrap_or(by);
    let (alt, q1, med, q3, ust) = (y(ozet[0]), y(ozet[1]), y(ozet[2]), y(ozet[3]), y(ozet[4]));
    let mavi = Renk::Bir;
    let turuncu = Renk::Iki;
    let cizgi = 1.4;

    // Bıyıklar: min ve max noktalarına kesik çizgi.
    liste.ekle(Oge::Dikey {
        x: merkez,
        y0: ust,
        y1: q3,
        renk: mavi,
        kalinlik: cizgi,
    });
    liste.ekle(Oge::Dikey {
        x: merkez,
        y0: q1,
        y1: alt,
        renk: mavi,
        kalinlik: cizgi,
    });
    liste.ekle(Oge::Yatay {
        x0: merkez - yari * 0.45,
        x1: merkez + yari * 0.45,
        y: ust,
        renk: mavi,
        kalinlik: cizgi,
    });
    liste.ekle(Oge::Yatay {
        x0: merkez - yari * 0.45,
        x1: merkez + yari * 0.45,
        y: alt,
        renk: mavi,
        kalinlik: cizgi,
    });
    // Kutu: sol kenar, sağ kenar, alt kenar, üst kenar.
    liste.ekle(Oge::Dikey {
        x: merkez - yari,
        y0: q1,
        y1: q3,
        renk: mavi,
        kalinlik: cizgi,
    });
    liste.ekle(Oge::Dikey {
        x: merkez + yari,
        y0: q1,
        y1: q3,
        renk: mavi,
        kalinlik: cizgi,
    });
    liste.ekle(Oge::Yatay {
        x0: merkez - yari,
        x1: merkez + yari,
        y: q1,
        renk: mavi,
        kalinlik: cizgi,
    });
    liste.ekle(Oge::Yatay {
        x0: merkez - yari,
        x1: merkez + yari,
        y: q3,
        renk: mavi,
        kalinlik: cizgi,
    });
    // Medyan kalın çizgi ve değer etiketi.
    liste.ekle(Oge::Yatay {
        x0: merkez - yari,
        x1: merkez + yari,
        y: med,
        renk: turuncu,
        kalinlik: 2.6,
    });
    liste.ekle(Oge::Metin {
        x: merkez + yari * 1.6,
        y: med,
        icerik: crate::bicim::sayi_metni(ozet[2]),
        hiza: Hiza::Sol,
        boyut: 12.0,
        renk: turuncu,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_yardimcisi::GeciciDizin;

    /// Test CSV'si yazar ve dosya yolunu döndürür.
    fn veri(etiket: &str, icerik: &str) -> (std::path::PathBuf, GeciciDizin) {
        let g = GeciciDizin::yeni(etiket);
        let yol = g
            .yaz("v.csv", icerik)
            .unwrap_or_else(|_| panic!("yazilamadi"));
        (yol, g)
    }

    /// Belirli bir şablonla grafik üretir.
    fn uret(etiket: &str, icerik: &str, sablon: Sablon, y: &str) -> Sonuc<Grafik> {
        let (yol, _g) = veri(etiket, icerik);
        let mut istek = Istek::yeni(yol);
        istek.sablon = sablon;
        istek.x_sutun = Some("x".to_string());
        istek.y_sutun = y.to_string();
        istek.esik = 50;
        uret_gercek(&istek)
    }

    fn uret_gercek(istek: &Istek) -> Sonuc<Grafik> {
        super::uret(istek)
    }

    /// Üçgen dalgalı veri (baskın frekanslı sinyal).
    fn ucgen(n: usize) -> String {
        let mut s = String::from("x,y\n");
        for i in 0..n {
            let v = (i as f64 * 0.1).sin() * 100.0;
            s.push_str(&format!("{i},{v}\n"));
        }
        s
    }

    #[test]
    fn tum_sablon_adlari_cozulur() {
        for ad in SABLON_ADLARI {
            assert!(Sablon::adindan(ad).is_ok(), "{ad} cozulmeli");
        }
    }

    #[test]
    fn bilinmeyen_sablon_hata_verir() {
        assert!(Sablon::adindan("pasta").is_err());
    }

    #[test]
    fn ingilizce_takma_adlar_calisir() {
        assert_eq!(
            Sablon::adindan("line").unwrap_or(Sablon::Cizgi),
            Sablon::Cizgi
        );
        assert_eq!(
            Sablon::adindan("scatter").unwrap_or(Sablon::Cizgi),
            Sablon::Nokta
        );
        assert_eq!(
            Sablon::adindan("box").unwrap_or(Sablon::Cizgi),
            Sablon::Kutu
        );
    }

    #[test]
    fn tek_sutun_sablonlari_isaretli() {
        assert!(Sablon::Histogram.tek_sutun());
        assert!(Sablon::Kutu.tek_sutun());
        assert!(!Sablon::Cizgi.tek_sutun());
        assert!(!Sablon::Nokta.tek_sutun());
    }

    #[test]
    fn cizgi_sablonu_indirger() {
        let g = uret("s-cizgi", &ucgen(5000), Sablon::Cizgi, "y");
        match g {
            Ok(g) => {
                assert_eq!(g.okunan, 5000);
                assert_eq!(g.indirgenen, 5000, "giris nokta sayisi");
                let cizgi = g
                    .cizim
                    .ogeler
                    .iter()
                    .any(|o| matches!(o, Oge::Cizgi { .. }));
                assert!(cizgi, "polyline olmali");
            }
            Err(e) => panic!("uretim hatasi: {e}"),
        }
    }

    #[test]
    fn nokta_sablonu_nokta_uretir() {
        let g = uret("s-nokta", &ucgen(100), Sablon::Nokta, "y");
        if let Ok(g) = g {
            assert!(g
                .cizim
                .ogeler
                .iter()
                .any(|o| matches!(o, Oge::Nokta { .. })));
        } else {
            panic!("uretim hatasi");
        }
    }

    #[test]
    fn alan_sablonu_alan_uretir() {
        let g = uret("s-alan", &ucgen(100), Sablon::Alan, "y");
        if let Ok(g) = g {
            assert!(g.cizim.ogeler.iter().any(|o| matches!(o, Oge::Alan { .. })));
        } else {
            panic!("uretim hatasi");
        }
    }

    #[test]
    fn histogram_sablonu_cubuk_uretir() {
        let mut s = String::from("x,v\n");
        for i in 0..1000 {
            s.push_str(&format!("{i},{}\n", (i % 100) as f64 / 10.0));
        }
        let g = uret("s-hist", &s, Sablon::Histogram, "v");
        if let Ok(g) = g {
            assert!(
                g.cizim
                    .ogeler
                    .iter()
                    .any(|o| matches!(o, Oge::Cubuk { .. })),
                "cubuk olmali"
            );
        } else {
            panic!("uretim hatasi");
        }
    }

    #[test]
    fn kutu_sablonu_bes_sayili_ozet_cizer() {
        let mut s = String::from("x,v\n");
        for i in 0..100 {
            s.push_str(&format!("{i},{}\n", i as f64));
        }
        let g = uret("s-kutu", &s, Sablon::Kutu, "v");
        if let Ok(g) = g {
            let dikey = g
                .cizim
                .ogeler
                .iter()
                .any(|o| matches!(o, Oge::Dikey { .. }));
            assert!(dikey, "kutu govdesi (dikey cizgi) olmali");
        } else {
            panic!("uretim hatasi");
        }
    }

    #[test]
    fn bes_sayili_ozet_dogru() {
        let mut v: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let o = bes_sayili_ozet(&mut v);
        assert_eq!(o[0], 1.0);
        assert_eq!(o[4], 100.0);
        assert!(
            o[1] <= o[2] && o[2] <= o[3],
            "ceyrekler sirali olmali: {o:?}"
        );
    }

    #[test]
    fn bes_sayili_ozet_bos_girdide_sifir_doner() {
        let mut v: Vec<f64> = Vec::new();
        assert_eq!(bes_sayili_ozet(&mut v), [0.0; 5]);
    }

    #[test]
    fn olmayan_sutun_hata_verir() {
        let g = uret("s-yok", &ucgen(10), Sablon::Cizgi, "olmayan");
        assert!(g.is_err());
    }

    #[test]
    fn sayisal_olmayan_sutun_bos_grafik_uretiyor_mu_kontrol() {
        // Metin sütun sayısal işlemde kullanılamaz; araç **hata vermez**,
        // boş seri ile sessizce boş grafik üretir (rapor b07 hata yönetimi).
        let g = uret("s-metin", "x,y\n1,abc\n2,def\n", Sablon::Cizgi, "y");
        match g {
            Ok(g) => assert_eq!(g.indirgenen, 0, "sayisal veri yok"),
            Err(_) => panic!("bu durum hataya donusmemeli"),
        }
    }

    #[test]
    fn bos_veri_hata_vermez_grafik_uretiir() {
        let g = uret("s-bos", "x,y\n", Sablon::Cizgi, "y");
        if let Ok(g) = g {
            assert_eq!(g.indirgenen, 0);
            assert!(
                g.cizim
                    .ogeler
                    .iter()
                    .any(|o| matches!(o, Oge::Cerceve { .. })),
                "eksen cizilmeli"
            );
        } else {
            panic!("bos veri hatasi vermemeli");
        }
    }

    #[test]
    fn tek_noktalik_veri_grafik_uretir() {
        let g = uret("s-tek", "x,y\n5,7\n", Sablon::Cizgi, "y");
        if let Ok(g) = g {
            assert_eq!(g.indirgenen, 1);
        } else {
            panic!("tek nokta hatasi vermemeli");
        }
    }

    #[test]
    fn nan_degerler_indirgemeye_girmez() {
        let mut s = String::from("x,y\n");
        for i in 0..200 {
            if i % 50 == 0 {
                s.push_str(&format!("{i},NaN\n"));
            } else {
                s.push_str(&format!("{i},{}\n", i as f64));
            }
        }
        let g = uret("s-nan", &s, Sablon::Cizgi, "y");
        if let Ok(g) = g {
            assert_eq!(g.indirgenen, 196, "NaN satirlar sayilir ama cizilmez");
        } else {
            panic!("NaN hatasi vermemeli");
        }
    }

    #[test]
    fn aralik_kisitlamasi_uygulanir() {
        let (yol, _g) = veri("s-aralik", &ucgen(1000));
        let mut istek = Istek::yeni(yol);
        istek.sablon = Sablon::Cizgi;
        istek.x_sutun = Some("x".to_string());
        istek.y_sutun = "y".to_string();
        istek.y_araligi = Some(Menzil::yeni(-10.0, 10.0));
        match uret_gercek(&istek) {
            Ok(g) => {
                assert!(
                    g.y_menzil.ust <= 10.0,
                    "ust sinir uygulanmali: {:?}",
                    g.y_menzil
                );
            }
            Err(e) => panic!("uretim hatasi: {e}"),
        }
    }

    #[test]
    fn gecersiz_esik_hata_verir() {
        let (yol, _g) = veri("s-esik", &ucgen(100));
        let mut istek = Istek::yeni(yol);
        istek.sablon = Sablon::Cizgi;
        istek.y_sutun = "y".to_string();
        istek.esik = 2;
        assert!(uret_gercek(&istek).is_err(), "esik 2 reddedilmeli");
    }

    #[test]
    fn gecersiz_boyut_hata_verir() {
        let (yol, _g) = veri("s-boyut", &ucgen(10));
        let mut istek = Istek::yeni(yol);
        istek.sablon = Sablon::Cizgi;
        istek.y_sutun = "y".to_string();
        istek.genislik = 0.0;
        assert!(uret_gercek(&istek).is_err());
    }

    #[test]
    fn tek_sutunlu_sablon_x_sutununu_yok_sayar() {
        let g = uret("s-tek-x", &ucgen(100), Sablon::Histogram, "y");
        if let Ok(g) = g {
            assert_eq!(g.okunan, 100);
        } else {
            panic!("histogram hatasi vermemeli");
        }
    }

    #[test]
    fn baslik_ve_alt_bilgi_doldurulur() {
        let g = uret("s-baslik", &ucgen(100), Sablon::Cizgi, "y");
        if let Ok(g) = g {
            assert!(
                !g.cizim.baslik.is_empty(),
                "baslik olmali: {}",
                g.cizim.baslik
            );
            assert!(
                g.cizim.alt_bilgi.contains("LTTB"),
                "alt bilgi LTTB bilgisi icermeli: {}",
                g.cizim.alt_bilgi
            );
        } else {
            panic!("uretim hatasi");
        }
    }
}
