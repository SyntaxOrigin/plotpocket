//! Hata tipleri ve bunların `Display` uygulamaları.
//!
//! Sorumluluğu, tüm modüllerin ortak kullandığı tek hata sınıfını tanımlamaktır.
//! `thiserror` bağımlılığı yasak olduğu için (`WORKER_CONTRACT.md` § 4.3)
//! `Display` ve `Error` uygulamaları elle yazılmıştır.
//!
//! Tüm hatalar kullanıcı girdisinden veya dosya sisteminden türetilir; hiçbir
//! kullanıcı girdisi yolu `panic!` üretmez. `panic!` yalnızca programcı hatası
//! (invariant bozulması) için kabul edilir ve bu crate'te böyle bir yol yoktur.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// PlotPocket'in tüm modüllerinde döndürülen hata tipi.
#[derive(Debug)]
#[non_exhaustive]
pub enum Hata {
    /// Dosya üzerinde G/Ç işlemi başarısız oldu.
    Io {
        /// Yapılmaya çalışılan işlemin kısa adı (ör. `"acma"`, `"okuma"`).
        islem: &'static str,
        /// İşlemin uygulandığı yol.
        yol: PathBuf,
        /// Altta yatan `std::io` hatası.
        kaynak: io::Error,
    },
    /// Dosya adından ve içerikten biçim belirlenemedi.
    BilinmeyenBicim {
        /// İncelenen yol.
        yol: PathBuf,
    },
    /// `--ayrac` ile verilen değer geçerli bir bayt değil.
    GecersizAyrac {
        /// Kullanıcının verdiği değer.
        deger: String,
    },
    /// İstenen sütun tabloda yok.
    SutunYok {
        /// Aranan sütun adı.
        sutun: String,
        /// Tablodaki sütun sayısı.
        sutun_sayisi: usize,
    },
    /// Sütun sayısal değil, indirgemede veya profillemede kullanılamaz.
    SutunSayisalDegil {
        /// Sütun adı.
        sutun: String,
        /// Sütunun çıkarılan tipi.
        tip: &'static str,
    },
    /// Satır alan sayısı başlıkla uyuşmuyor (bozuk satır; atlanır ve sayılır).
    BozukSatir {
        /// Satırın 1 tabanlı numarası.
        satir: u64,
        /// Bulunan alan sayısı.
        bulunan: usize,
        /// Beklenen alan sayısı.
        beklenen: usize,
    },
    /// JSONL satırı geçerli bir JSON nesnesi değil (atlanır ve sayılır).
    BozukJson {
        /// Satırın 1 tabanlı numarası.
        satir: u64,
        /// Ayrıştırıcının verdiği kısa açıklama.
        mesaj: String,
    },
    /// Aşağı yukarı aralık geçersiz (alt > üst).
    GecersizAralik {
        /// İstenen alt sınır.
        alt: f64,
        /// İstenen üst sınır.
        ust: f64,
    },
    /// Genişlik veya yükseklik sıfır ya da aşırı büyük.
    GecersizBoyut {
        /// Verilen genişlik.
        genislik: usize,
        /// Verilen yükseklik.
        yukseklik: usize,
    },
    /// Eşik değeri (indirgeme nokta bütçesi) 3'ten küçük.
    GecersizEsik {
        /// Verilen eşik.
        esik: usize,
    },
    /// Şablon adı bilinmiyor.
    BilinmeyenSablon {
        /// Kullanıcının verdiği ad.
        ad: String,
    },
    /// Dışa aktarım hedefi yazılamadı.
    CiktiHatasi {
        /// Hedef yol.
        yol: PathBuf,
        /// Altta yatan `std::io` hatası.
        kaynak: io::Error,
    },
}

impl fmt::Display for Hata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hata::Io { islem, yol, kaynak } => {
                write!(f, "{} işlemi başarısız ({}): {}", islem, yol.display(), kaynak)
            }
            Hata::BilinmeyenBicim { yol } => write!(
                f,
                "biçim belirlenemedi ({}): uzantı tanınmıyor ve içerik CSV/TSV/JSONL olarak ayrıştırılamadı",
                yol.display()
            ),
            Hata::GecersizAyrac { deger } => {
                write!(f, "geçersiz ayraç {deger:?}: tek bir bayt olmalı (ör. `,` veya `\\t`)")
            }
            Hata::SutunYok { sutun, sutun_sayisi } => {
                write!(f, "sütun bulunamadı: {sutun:?} (dosyada {sutun_sayisi} sütun var)")
            }
            Hata::SutunSayisalDegil { sutun, tip } => write!(
                f,
                "sütun sayısal değil: {sutun:?} (tip: {tip}); sayısal işlem için uygun sütun seçin"
            ),
            Hata::BozukSatir { satir, bulunan, beklenen } => write!(
                f,
                "bozuk satır {satir}: {bulunan} alan bulundu, {beklenen} bekleniyordu"
            ),
            Hata::BozukJson { satir, mesaj } => {
                write!(f, "bozuk JSONL satırı {satir}: {mesaj}")
            }
            Hata::GecersizAralik { alt, ust } => {
                write!(f, "geçersiz aralık: alt ({alt}) üst ({ust}) değerinden büyük")
            }
            Hata::GecersizBoyut { genislik, yukseklik } => {
                write!(f, "geçersiz boyut: {genislik}x{yukseklik} (her ikisi de 1..=2000 olmalı)")
            }
            Hata::GecersizEsik { esik } => {
                write!(f, "geçersiz eşik: {esik} (LTTB için en az 3 olmalı)")
            }
            Hata::BilinmeyenSablon { ad } => write!(
                f,
                "bilinmeyen şablon: {ad:?}; `plotpocket templates` ile listeyi görebilirsiniz"
            ),
            Hata::CiktiHatasi { yol, kaynak } => {
                write!(f, "çıktı yazılamadı ({}): {}", yol.display(), kaynak)
            }
        }
    }
}

impl Error for Hata {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Hata::Io { kaynak, .. } | Hata::CiktiHatasi { kaynak, .. } => Some(kaynak),
            _ => None,
        }
    }
}

/// Sonuç takma adı; modüllerin imzasını kısa tutar.
pub type Sonuc<T> = Result<T, Hata>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hata_gosterimi_aciklama_iceriyor() {
        let h = Hata::SutunYok {
            sutun: "y".into(),
            sutun_sayisi: 3,
        };
        let m = h.to_string();
        assert!(m.contains("y"), "mesaj sütun adını içermeli: {m}");
        assert!(m.contains('3'), "mesaj sütun sayısını içermeli: {m}");
    }

    #[test]
    fn hata_kaynagi_io_hatasina_isaret_eder() {
        let h = Hata::Io {
            islem: "acma",
            yol: PathBuf::from("x.csv"),
            kaynak: io::Error::new(io::ErrorKind::NotFound, "yok"),
        };
        assert!(h.source().is_some(), "Io hatasında kaynak olmalı");
    }

    #[test]
    fn kaynaksiz_hatada_kaynak_yoktur() {
        let h = Hata::GecersizAyrac { deger: ",".into() };
        assert!(h.source().is_none());
    }
}
