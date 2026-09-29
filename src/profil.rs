//! Küçük örnekleme ile hızlı sütun profilleme.
//!
//! Sorumluluğu, dosyanın **tamamını okumadan** sütunların tipini, boş oranını,
//! sayısal aralığını ve metin sütunlarında en sık değerleri çıkarmaktır.
//!
//! # Neden örnekleme
//!
//! Profil, tip çıkarımı için dosyanın **başından** yalnızca `ornek_kayit` kadar
//! kayıt okur; dosyanın tamamı taranmaz. `plotpocket profile --tam` verildiğinde
//! `toplam_kayit_say` ile **ayrı bir tam sayım geçişi** yapılır ve sonuç kesin
//! değere bağlanır; varsayılan çalışmada `toplam_kayit: None` kalır ve
//! `kayit_tahmini` dosya boyutundan türetilir.
//!
//! # Tip çıkarım kuralı
//!
//! Her sütun için üç bayrak tutulur: `metin_goruldu`, `ondalik_goruldu`,
//! `sayisal_goruldu`. Sonuç şöyle çıkar:
//!
//! | Koşul | Sonuç |
//! |---|---|
//! | En az bir sayısal olmayan dolu alan | `Metin` |
//! | Sayısal alanlardan en az biri ondalıklı | `Kayan` |
//! | En az bir sayısal alan, hepsi tam | `TamSayi` |
//! | Hiç dolu alan yok (tamamı boş) | `Metin` |

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::ayristirici::{AcSecenekleri, Alan, Okuyucu, Satir};
use crate::hata::Sonuc;

/// Varsayılan örnek: 1.000 kayıt (tip çıkarımı için fazlasıyla yeterli).
pub const VARSAYILAN_ORNEK: usize = 1000;

/// En sık değer tablosunda tutulan azami farklı metin sayısı.
const FARKLI_SINIR: usize = 512;

/// `2^53` sınırı: bu değerin üstündeki tam sayılar `f64` olarak tam temsil edilemez.
const TAM_SINIR: f64 = 9.007_199_254_740_992e15;

/// Sütunun çıkarılan tipi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SutunTipi {
    /// Tüm dolu alanlar tam sayıya ayrıştırıldı.
    TamSayi,
    /// En az bir dolu alan kayan noktaya ayrıştırıldı.
    Kayan,
    /// En az bir dolu alan sayısal değil (ya da sütun tamamen boş).
    Metin,
}

impl SutunTipi {
    /// `profile` çıktısındaki adı.
    pub fn ad(self) -> &'static str {
        match self {
            SutunTipi::TamSayi => "tam-sayi",
            SutunTipi::Kayan => "kayan",
            SutunTipi::Metin => "metin",
        }
    }
}

/// Tek sütunun profili.
#[derive(Debug, Clone, Serialize)]
pub struct SutunProfili {
    /// Sütun adı.
    pub ad: String,
    /// Sütun indeksi (0 tabanlı).
    pub indeks: usize,
    /// Çıkarılan tip.
    pub tip: SutunTipi,
    /// Örnekteki kayıt sayısı.
    pub gozlenen: usize,
    /// Boş alan sayısı.
    pub bos: usize,
    /// Boş alan oranı (`0.0..=1.0`).
    pub bos_orani: f64,
    /// Sayısal sütunlarda en küçük değer; metin sütunlarda `None`.
    pub en_kucuk: Option<f64>,
    /// Sayısal sütunlarda en büyük değer; metin sütunlarda `None`.
    pub en_buyuk: Option<f64>,
    /// Sayısal sütunlarda ortalama; metin sütunlarda `None`.
    pub ortalama: Option<f64>,
    /// Sonlu olmayan değer sayısı (`NaN`, `±inf`).
    pub sonlu_degil: usize,
    /// Örnekte görülen farklı metin değeri sayısı.
    pub farkli_deger: usize,
    /// Metin sütunlarda en sık değerler (frekansla birlikte, en fazla 5).
    pub en_sik: Vec<(String, usize)>,
}

impl SutunProfili {
    /// Sütunun sayısal işlemlerde (grafik, LTTB) kullanılabilir olup olmadığı.
    pub fn sayisal(&self) -> bool {
        matches!(self.tip, SutunTipi::TamSayi | SutunTipi::Kayan)
    }
}

/// Dosyanın bütününe ilişkin profil.
#[derive(Debug, Clone, Serialize)]
pub struct DosyaProfili {
    /// İncelenen dosyanın adı.
    pub dosya: String,
    /// Tanınan biçim.
    pub bicim: String,
    /// Alan ayracı (JSON biçiminde `0`).
    pub ayrac: u8,
    /// Sütun adları.
    pub basliklar: Vec<String>,
    /// Profilde **okunan** kayıt sayısı.
    pub ornek_kayit: usize,
    /// Toplam kayıt sayısı; tam sayım yapılmadıysa `None`.
    pub toplam_kayit: Option<u64>,
    /// Dosya boyutundan türetilen kayıt sayısı tahmini.
    pub kayit_tahmini: u64,
    /// Dosya boyutu (bayt).
    pub dosya_boyutu: u64,
    /// Atlanan bozuk kayıt sayısı.
    pub bozuk_kayit: u64,
    /// Sütun profilleri.
    pub sutunlar: Vec<SutunProfili>,
}

/// Dosyayı örnekleyerek profiller.
///
/// `ornek_kayit == 0` ise `VARSAYILAN_ORNEK` kullanılır.
pub fn profille(yol: &Path, ac: &AcSecenekleri, ornek_kayit: usize) -> Sonuc<DosyaProfili> {
    let ornek = if ornek_kayit == 0 {
        VARSAYILAN_ORNEK
    } else {
        ornek_kayit
    };
    let mut okuyucu = Okuyucu::ac(yol, ac)?;
    let basliklar = okuyucu.basliklar().to_vec();
    let sutun_sayisi = okuyucu.sutun_sayisi();
    let bicim = okuyucu.bicim().ad().to_string();
    let ayrac = okuyucu.ayrac();

    let mut sayaclar: Vec<Durum> = (0..sutun_sayisi).map(|_| Durum::yeni()).collect();
    let mut gozlenen = 0usize;
    while gozlenen < ornek {
        match okuyucu.sonraki_satir()? {
            None => break,
            Some(s) => {
                durumlari_doldur(&mut sayaclar, &s);
                gozlenen += 1;
            }
        }
    }
    let bozuk_kayit = okuyucu.bozuk_satir_sayisi();
    drop(okuyucu);

    let sutunlar: Vec<SutunProfili> = sayaclar
        .iter()
        .enumerate()
        .map(|(i, d)| {
            d.bitir(
                basliklar.get(i).map(String::as_str).unwrap_or("?"),
                i,
                gozlenen,
            )
        })
        .collect();

    let boyut = std::fs::metadata(yol).map(|m| m.len()).unwrap_or(0);
    // Ortalama kayıt uzunluğundan dosya boyutuna bölerek kayıt sayısı tahmini.
    let kayit_tahmini = if gozlenen > 0 && boyut > 0 {
        let ortalama = boyut as f64 / gozlenen as f64;
        (boyut as f64 / ortalama.max(1.0)) as u64
    } else {
        0
    };

    Ok(DosyaProfili {
        dosya: yol
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| yol.display().to_string()),
        bicim,
        ayrac,
        basliklar,
        ornek_kayit: gozlenen,
        toplam_kayit: None,
        kayit_tahmini,
        dosya_boyutu: boyut,
        bozuk_kayit,
        sutunlar,
    })
}

/// Tam kayıt sayımı yapar (dosyayı baştan sona okur).
pub fn toplam_kayit_say(yol: &Path, ac: &AcSecenekleri) -> Sonuc<u64> {
    let mut okuyucu = Okuyucu::ac(yol, ac)?;
    let mut adet = 0u64;
    while okuyucu.sonraki_satir()?.is_some() {
        adet += 1;
    }
    Ok(adet)
}

/// Tüm sütun sayaçlarını bir kayıtla besler.
fn durumlari_doldur(sayaclar: &mut [Durum], s: &Satir<'_>) {
    for (i, durum) in sayaclar.iter_mut().enumerate() {
        durum.ekle(s, i);
    }
}

/// Tek sütunun kademeli tip/istatistik toplayıcısı.
struct Durum {
    metin_goruldu: bool,
    ondalik_goruldu: bool,
    sayisal_adet: usize,
    bos: usize,
    sonlu_degil: usize,
    en_kucuk: f64,
    en_buyuk: f64,
    toplam: f64,
    farkli: HashMap<String, usize>,
    farkli_tam: bool,
}

impl Durum {
    fn yeni() -> Self {
        Durum {
            metin_goruldu: false,
            ondalik_goruldu: false,
            sayisal_adet: 0,
            bos: 0,
            sonlu_degil: 0,
            en_kucuk: f64::INFINITY,
            en_buyuk: f64::NEG_INFINITY,
            toplam: 0.0,
            farkli: HashMap::new(),
            farkli_tam: true,
        }
    }

    /// Kaydın `i`. alanını toplayıcıya ekler.
    fn ekle(&mut self, s: &Satir<'_>, i: usize) {
        let alan = match s.alanlar.get(i) {
            Some(a) => a,
            None => {
                self.bos += 1;
                return;
            }
        };
        if matches!(alan, Alan::Bos) {
            self.bos += 1;
            return;
        }
        match alan.sayi() {
            Some(v) => {
                if !v.is_finite() {
                    // `NaN` / `±inf` sayısal bir sütunla bağdaşmaz; metin sayılır.
                    self.sonlu_degil += 1;
                    self.metin_goruldu = true;
                    return;
                }
                self.sayisal_adet += 1;
                if v.fract() != 0.0 || v.abs() >= TAM_SINIR {
                    self.ondalik_goruldu = true;
                }
                self.en_kucuk = self.en_kucuk.min(v);
                self.en_buyuk = self.en_buyuk.max(v);
                self.toplam += v;
            }
            None => {
                self.metin_goruldu = true;
                if self.farkli_tam && self.farkli.len() < FARKLI_SINIR {
                    *self.farkli.entry(s.metin(i)).or_insert(0) += 1;
                } else if self.farkli_tam {
                    // Sınır aşıldı: sayaç artık tam değildir, işaretlenir.
                    self.farkli_tam = false;
                }
            }
        }
    }

    fn tip(&self) -> SutunTipi {
        if self.metin_goruldu {
            SutunTipi::Metin
        } else if self.ondalik_goruldu {
            SutunTipi::Kayan
        } else if self.sayisal_adet > 0 {
            SutunTipi::TamSayi
        } else {
            SutunTipi::Metin
        }
    }

    fn bitir(&self, ad: &str, indeks: usize, gozlenen: usize) -> SutunProfili {
        let (k, b, o) = if self.sayisal_adet > 0 {
            (
                Some(self.en_kucuk),
                Some(self.en_buyuk),
                Some(self.toplam / self.sayisal_adet as f64),
            )
        } else {
            (None, None, None)
        };
        let mut en_sik: Vec<(String, usize)> =
            self.farkli.iter().map(|(k, v)| (k.clone(), *v)).collect();
        en_sik.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        en_sik.truncate(5);
        SutunProfili {
            ad: ad.to_string(),
            indeks,
            tip: self.tip(),
            gozlenen,
            bos: self.bos,
            bos_orani: if gozlenen == 0 {
                0.0
            } else {
                self.bos as f64 / gozlenen as f64
            },
            en_kucuk: k,
            en_buyuk: b,
            ortalama: o,
            sonlu_degil: self.sonlu_degil,
            farkli_deger: if self.farkli_tam {
                self.farkli.len()
            } else {
                FARKLI_SINIR
            },
            en_sik,
        }
    }
}

/// Profili insan okunur metne çevirir.
pub fn metin(p: &DosyaProfili) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(512);
    let ayrac_metin = if p.ayrac == 0 {
        "-".to_string()
    } else {
        (p.ayrac as char).to_string()
    };
    let _ = writeln!(s, "dosya        : {}", p.dosya);
    let _ = writeln!(s, "bicim        : {}", p.bicim);
    let _ = writeln!(s, "ayrac        : {ayrac_metin}");
    let _ = writeln!(s, "boyut        : {} bayt", p.dosya_boyutu);
    let _ = writeln!(s, "ornek kayit  : {}", p.ornek_kayit);
    match p.toplam_kayit {
        Some(t) => {
            let _ = writeln!(s, "toplam kayit : {t} (tam sayim)");
        }
        None => {
            let _ = writeln!(
                s,
                "kayit tahmini: {} (tahmin; --tam ile sayilir)",
                p.kayit_tahmini
            );
        }
    }
    let _ = writeln!(s, "bozuk kayit  : {}", p.bozuk_kayit);
    let _ = writeln!(s, "sutun sayisi : {}", p.basliklar.len());
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "{:<4} {:<18} {:<9} {:>6} {:>6} {:>13} {:>13}",
        "#", "sutun", "tip", "bos%", "sonsuz", "en kucuk", "en buyuk"
    );
    for c in &p.sutunlar {
        let k = c
            .en_kucuk
            .map(crate::bicim::sayi_metni)
            .unwrap_or_else(|| "-".to_string());
        let b = c
            .en_buyuk
            .map(crate::bicim::sayi_metni)
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            s,
            "{:<4} {:<18} {:<9} {:>5.1}% {:>6} {:>13} {:>13}",
            c.indeks,
            kisalt(&c.ad, 18),
            c.tip.ad(),
            c.bos_orani * 100.0,
            c.sonlu_degil,
            k,
            b
        );
    }
    s
}

/// Sütun adını verilen genişliğe kısaltır; gerekiyorsa `…` ile biter.
fn kisalt(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let ilk: String = s.chars().take(n.saturating_sub(1)).collect();
    format!("{ilk}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ayristirici::AcSecenekleri;
    use crate::test_yardimcisi::GeciciDizin;
    use std::io::Write as _;
    use std::path::PathBuf;

    fn gecici(ad: &str, icerik: &str) -> (PathBuf, GeciciDizin) {
        let g = GeciciDizin::yeni(ad);
        let yol = g.yol().join("v.csv");
        let mut f = std::fs::File::create(&yol).unwrap_or_else(|_| panic!("olusturulamadı"));
        f.write_all(icerik.as_bytes())
            .unwrap_or_else(|_| panic!("yazilamadi"));
        drop(f);
        (yol, g)
    }

    #[test]
    fn tam_sayi_tipi_cikarilir() {
        let (yol, _g) = gecici("p-tam", "a\n1\n2\n3\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        if let Ok(p) = p {
            assert_eq!(p.sutunlar[0].tip, SutunTipi::TamSayi);
        } else {
            panic!("profil alinamadi");
        }
    }

    #[test]
    fn tip_kademeleri_dogru_isler() {
        let (yol, _g) = gecici("p-tam2", "a,b,c,d\n1,1.5,x,\n2,2.5,y,\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        match p {
            Ok(p) => {
                assert_eq!(p.sutunlar[0].tip, SutunTipi::TamSayi);
                assert_eq!(p.sutunlar[1].tip, SutunTipi::Kayan);
                assert_eq!(p.sutunlar[2].tip, SutunTipi::Metin);
                assert_eq!(p.sutunlar[3].tip, SutunTipi::Metin);
            }
            Err(_) => panic!("profil alinamadi"),
        }
    }

    #[test]
    fn bos_orani_hesaplanir() {
        // Tek sütunlu dosyada boş satır geçerli (boş alan) bir kayıttır.
        let (yol, _g) = gecici("p-bos", "a\n1\n\n\n4\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        if let Ok(p) = p {
            assert_eq!(p.ornek_kayit, 4, "dort kayit okunmali");
            assert_eq!(p.sutunlar[0].bos, 2, "iki bos alan olmali");
            assert!(
                (p.sutunlar[0].bos_orani - 0.5).abs() < 1e-9,
                "oran 0.5: {}",
                p.sutunlar[0].bos_orani
            );
        } else {
            panic!("profil alinamadi");
        }
    }

    #[test]
    fn cok_sutunlu_dosyada_bos_satir_bozuk_sayilir() {
        // İki sütunlu dosyada boş satır alan sayısı tutmaz; bozuk sayılır.
        let (yol, _g) = gecici("p-bos2", "a,b\n1,2\n\n3,4\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        if let Ok(p) = p {
            assert_eq!(p.ornek_kayit, 2, "gecerli iki kayit kalmali");
            assert_eq!(p.bozuk_kayit, 1, "bos satir bozuk sayilmali");
        } else {
            panic!("profil alinamadi");
        }
    }

    #[test]
    fn sayisal_istatistikler_dogru() {
        let (yol, _g) = gecici("p-ist", "a\n1\n2\n3\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        if let Ok(p) = p {
            assert_eq!(p.sutunlar[0].en_kucuk, Some(1.0));
            assert_eq!(p.sutunlar[0].en_buyuk, Some(3.0));
            assert_eq!(p.sutunlar[0].ortalama, Some(2.0));
        } else {
            panic!("profil alinamadi");
        }
    }

    #[test]
    fn tam_sayim_yapilabilir() {
        let (yol, _g) = gecici("p-say", "a\n1\n2\n3\n4\n5\n");
        let n = toplam_kayit_say(&yol, &AcSecenekleri::default());
        assert_eq!(n.unwrap_or(0), 5);
    }

    #[test]
    fn metin_ciktisi_uretir() {
        let (yol, _g) = gecici("p-mt", "a\n1\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        if let Ok(p) = p {
            let m = metin(&p);
            assert!(m.contains("sutun sayisi"), "cikti eksik: {m}");
        } else {
            panic!("profil alinamadi");
        }
    }

    #[test]
    fn ondalik_dondurulur() {
        let (yol, _g) = gecici("p-on", "a\n0.1\n0.2\n");
        let p = profille(&yol, &AcSecenekleri::default(), 0);
        if let Ok(p) = p {
            assert_eq!(p.sutunlar[0].tip, SutunTipi::Kayan);
        } else {
            panic!("profil alinamadi");
        }
    }
}
