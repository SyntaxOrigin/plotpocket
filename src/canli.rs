//! Canlı takip (tail-benzeri) modu.
//!
//! Raporun `notify` yerine seçtiği yol: dosya periyodik olarak **yeniden
//! açılır**, boyutu karşılaştırılır ve yalnızca **yeni baytlar** okunur. Bu
//! yaklaşım `notify`/inoot kütüphanesi gerektirmez, platform bağımsızdır ve
//! USB ortamında da çalışır (rapor b06 "Dosya katmanı").
//!
//! # Büyüme ve kırpma
//!
//! - Dosya **büyürse**: yeni satırlar eklenir ve grafik yeniden üretilir.
//! - Dosya **küçülürse** (yeniden adlandırıldı, log rotasyonu): kuyruk sıfırlanır
//!   ve dosya baştan okunur. Bu davranış `donme` sayacıyla bildirilir.
//! - Dosya **değişmezse**: yalnızca süre ölçülür, G/Ç yapılmaz.
//!
//! # Bellek
//!
//! Bellek, dosya büyüdükçe sınırsız artmaz: kuyruk `kuyruk_tavani` noktaya
//! kırpılır. Kırpma **en eskiyi atarak** yapılır (yeni veri korunur) ve
//! `kirpildi` sayacı artar, böylece veri kaybı sessizce olmaz (rapor b08
//! "Canlı kuyruk: taşma kuralı açıkça tanımlanmalıdır").
//!
//! # Zaman
//!
//! Yalnızca `std::time` kullanılır (`notify` ve zaman kütüphaneleri yasak).

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ayristirici::{AcSecenekleri, Okuyucu, Satir};
use crate::hata::{Hata, Sonuc};

/// Varsayılan yoklama aralığı (500 ms).
pub const VARSAYILAN_ARALIK: Duration = Duration::from_millis(500);
/// Varsayılan kuyruk tavanı (200.000 nokta; rapor b08).
pub const VARSAYILAN_TAVAN: usize = 200_000;

/// Canlı takip durumu.
pub struct Takip {
    yol: PathBuf,
    ac: AcSecenekleri,
    xi: Option<usize>,
    yi: usize,
    okuyucu: Option<Okuyucu>,
    dosya_boyutu: u64,
    kuyruk: Vec<(f64, f64)>,
    tavan: usize,
    /// Başlangıçtan bu yana okunan yeni satır sayısı.
    pub toplam_yeni: u64,
    /// Kırpma nedeniyle atılan nokta sayısı.
    pub kirpildi: u64,
    /// Dosya küçüldüğünde artan sayaç (log rotasyonu).
    pub donme: u64,
    /// Atlanan bozuk satır sayısı.
    pub bozuk: u64,
    /// Son yoklamada dosyada yeni veri bulundu mu.
    pub degisti: bool,
}

impl Takip {
    /// Takipçiyi oluşturur ve dosyayı ilk kez baştan okur.
    ///
    /// # Hatalar
    ///
    /// Dosya açılamazsa `Hata::Io`, sütun yoksa `Hata::SutunYok` döner.
    pub fn baslat(
        yol: &Path,
        ac: AcSecenekleri,
        x_sutun: Option<String>,
        y_sutun: &str,
        tavan: usize,
    ) -> Sonuc<Self> {
        let tavan = if tavan == 0 { VARSAYILAN_TAVAN } else { tavan };
        let okuyucu = Okuyucu::ac(yol, &ac)?;
        let sutun_sayisi = okuyucu.sutun_sayisi();
        let yi = okuyucu
            .sutun_indeksi(y_sutun)
            .ok_or_else(|| Hata::SutunYok {
                sutun: y_sutun.to_string(),
                sutun_sayisi,
            })?;
        let xi = match x_sutun.as_deref() {
            None => None,
            Some(ad) => Some(okuyucu.sutun_indeksi(ad).ok_or_else(|| Hata::SutunYok {
                sutun: ad.to_string(),
                sutun_sayisi,
            })?),
        };
        let mut takip = Takip {
            yol: yol.to_path_buf(),
            ac,
            xi,
            yi,
            okuyucu: Some(okuyucu),
            dosya_boyutu: 0,
            kuyruk: Vec::new(),
            tavan,
            toplam_yeni: 0,
            kirpildi: 0,
            donme: 0,
            bozuk: 0,
            degisti: false,
        };
        // İlk okuma: başlık zaten okundu, gövde baştan tüketilir.
        takip.dosya_boyutu = takip.sonraki_konum();
        takip.hesapla_guncelle(true)?;
        // `toplam_yeni` yalnızca **izleme başladıktan sonra** okunan satırları
        // sayar; açılışta okunan gövde bu sayaca dahil edilmez.
        takip.toplam_yeni = 0;
        Ok(takip)
    }

    /// Dosyanın güncel boyutu (yoksa 0).
    fn sonraki_konum(&self) -> u64 {
        std::fs::metadata(&self.yol).map(|m| m.len()).unwrap_or(0)
    }

    /// Dosyayı yoklar ve yeni satırları kuyruğa ekler.
    ///
    /// Dönüş: dosyada yeni veri bulunduysa `true`.
    pub fn yokla(&mut self) -> Sonuc<bool> {
        let boyut = self.sonraki_konum();
        self.degisti = false;
        if boyut == self.dosya_boyutu {
            return Ok(false);
        }
        // Boyut **azaldıysa** dosya küçülmüş demektir: log rotasyonu, yeniden
        // adlandırma veya `truncate`. Eklenen bir dosyanın boyutu asla
        // azalmaz, bu yüzden ek bir tolerans gerekmez.
        if boyut < self.dosya_boyutu {
            // Rotasyon veya yeniden adlandırma: baştan oku.
            self.donme += 1;
            self.kuyruk.clear();
            self.okuyucu = Some(Okuyucu::ac(&self.yol, &self.ac)?);
            self.dosya_boyutu = self.satir_sonu_konumu();
            self.degisti = true;
            return self.hesapla_guncelle(true);
        }
        self.dosya_boyutu = boyut;
        self.degisti = true;
        self.hesapla_guncelle(false)
    }

    /// Okuyucu tamponunun kalanını okuyup kuyruğa ekler.
    fn hesapla_guncelle(&mut self, bastan: bool) -> Sonuc<bool> {
        if bastan && self.okuyucu.is_none() {
            self.okuyucu = Some(Okuyucu::ac(&self.yol, &self.ac)?);
        }
        let okuyucu = match self.okuyucu.as_mut() {
            Some(o) => o,
            None => return Ok(false),
        };
        let xi = self.xi;
        let yi = self.yi;
        let tavan = self.tavan;
        let mut yeni = 0u64;
        let mut kirp = 0u64;
        let mut hata: Option<Hata> = None;
        loop {
            match okuyucu.sonraki_satir() {
                Err(e) => {
                    hata = Some(e);
                    break;
                }
                Ok(None) => break,
                Ok(Some(s)) => {
                    if let Some((x, y)) = nokta_al(&s, xi, yi) {
                        kuyruga_ekle(&mut self.kuyruk, (x, y), tavan, &mut kirp);
                        yeni += 1;
                    }
                }
            }
        }
        self.bozuk = okuyucu.bozuk_satir_sayisi();
        self.toplam_yeni += yeni;
        self.kirpildi += kirp;
        if let Some(e) = hata {
            return Err(e);
        }
        Ok(yeni > 0)
    }

    /// Kuyruktaki (x, y) çiftlerinin kopyası.
    pub fn kuyruk(&self) -> &[(f64, f64)] {
        &self.kuyruk
    }

    /// Kuyruktaki nokta sayısı.
    pub fn nokta_sayisi(&self) -> usize {
        self.kuyruk.len()
    }

    /// Kuyruk tavanı.
    pub fn tavan(&self) -> usize {
        self.tavan
    }

    /// Satır sonu ile hizalanmış bayt konumu (dosya kırpılmışsa 0).
    fn satir_sonu_konumu(&self) -> u64 {
        match File::open(&self.yol) {
            Err(_) => 0,
            Ok(mut f) => {
                let boyut = f.seek(SeekFrom::End(0)).unwrap_or(0);
                // Son 64 KiB'nin son satır sonunu bul; dosya çok büyükse
                // okuyucu başlık + satır sonu kuralıyla zaten devam eder.
                let bas = boyut.saturating_sub(64 * 1024);
                if f.seek(SeekFrom::Start(bas)).is_err() {
                    return boyut;
                }
                let mut r = BufReader::with_capacity(8192, f);
                let mut satir = Vec::new();
                let mut son = bas;
                loop {
                    satir.clear();
                    match r.read_until(b'\n', &mut satir) {
                        Ok(0) => break,
                        Ok(n) => {
                            son += n as u64;
                            if son >= boyut {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                son
            }
        }
    }
}

/// Kuyruğa nokta ekler; tavanı aşarsa **en eskiyi** atar.
fn kuyruga_ekle(kuyruk: &mut Vec<(f64, f64)>, p: (f64, f64), tavan: usize, kirp: &mut u64) {
    if kuyruk.len() >= tavan {
        // En eski yüzde onu at (kayma maliyetini düşürür).
        let dusur = (tavan / 10).max(1);
        kuyruk.drain(0..dusur.min(kuyruk.len()));
        *kirp += dusur as u64;
    }
    kuyruk.push(p);
}

/// Kayıttan `(x, y)` çıkarır; sayısal olmayan veya sonlu olmayan alanlar yok sayılır.
fn nokta_al(s: &Satir<'_>, xi: Option<usize>, yi: usize) -> Option<(f64, f64)> {
    let y = s.sayi(yi)?;
    if !y.is_finite() {
        return None;
    }
    let x = match xi {
        None => s.numara as f64,
        Some(i) => {
            let v = s.sayi(i)?;
            if !v.is_finite() {
                return None;
            }
            v
        }
    };
    Some((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_yardimcisi::GeciciDizin;
    use std::io::Write as _;

    fn dosya_yaz(yol: &Path, icerik: &str) {
        let mut f = std::fs::File::create(yol).unwrap_or_else(|_| panic!("olusturulamadı"));
        f.write_all(icerik.as_bytes())
            .unwrap_or_else(|_| panic!("yazilamadi"));
    }

    fn kur(ad: &str, icerik: &str) -> (PathBuf, GeciciDizin) {
        let g = GeciciDizin::yeni(ad);
        let yol = g.yol().join("canli.csv");
        dosya_yaz(&yol, icerik);
        (yol, g)
    }

    #[test]
    fn baslangic_kuyrugu_doldurur() {
        let (yol, _g) = kur("t-bas", "a,b\n1,10\n2,20\n3,30\n");
        let t = Takip::baslat(&yol, AcSecenekleri::default(), Some("a".into()), "b", 100);
        if let Ok(t) = t {
            assert_eq!(t.nokta_sayisi(), 3);
            assert_eq!(t.kuyruk()[0], (1.0, 10.0));
        } else {
            panic!("takip baslatilamadi");
        }
    }

    #[test]
    fn dosya_buyuyunce_yeni_satirlar_eklenir() {
        let (yol, _g) = kur("t-buy", "a,b\n1,10\n");
        let mut t = Takip::baslat(&yol, AcSecenekleri::default(), Some("a".into()), "b", 100);
        if let Ok(ref mut t) = t {
            assert_eq!(t.nokta_sayisi(), 1);
            let mut ek = std::fs::OpenOptions::new()
                .append(true)
                .open(&yol)
                .unwrap_or_else(|_| panic!("acilamadi"));
            ek.write_all(b"2,20\n3,30\n")
                .unwrap_or_else(|_| panic!("yazilamadi"));
            let eklendi = t.yokla();
            assert!(eklendi.unwrap_or(false), "yeni satirlar okunmali");
            assert_eq!(t.nokta_sayisi(), 3);
        } else {
            panic!("takip baslatilamadi");
        }
    }

    #[test]
    fn degismeyen_dosyada_yoklama_bos_doner() {
        let (yol, _g) = kur("t-degismez", "a,b\n1,10\n");
        let mut t = Takip::baslat(&yol, AcSecenekleri::default(), Some("a".into()), "b", 100);
        if let Ok(ref mut t) = t {
            let once = t.nokta_sayisi();
            assert!(!t.yokla().unwrap_or(true));
            assert_eq!(t.nokta_sayisi(), once);
        } else {
            panic!("takip baslatilamadi");
        }
    }

    #[test]
    fn tavan_asiilirsa_eski_noktalar_kirpilir() {
        let (yol, _g) = kur("t-tavan", "a,b\n1,10\n2,20\n3,30\n4,40\n5,50\n");
        let mut t = Takip::baslat(&yol, AcSecenekleri::default(), Some("a".into()), "b", 2);
        if let Ok(ref mut t) = t {
            assert!(
                t.nokta_sayisi() <= 2,
                "tavan asilmamali: {}",
                t.nokta_sayisi()
            );
            assert!(t.kirpildi > 0, "kirpma sayaci artmali");
            // Yeni veri korunur.
            assert_eq!(t.kuyruk().last().copied(), Some((5.0, 50.0)));
        } else {
            panic!("takip baslatilamadi");
        }
    }

    #[test]
    fn dosya_kuculurse_donme_sayaci_artar() {
        let (yol, _g) = kur("t-kuculme", "a,b\n1,10\n2,20\n3,30\n4,40\n5,50\n");
        let mut t = Takip::baslat(&yol, AcSecenekleri::default(), Some("a".into()), "b", 100);
        if let Ok(ref mut t) = t {
            dosya_yaz(&yol, "a,b\n9,90\n");
            t.yokla().unwrap_or_else(|e| panic!("yoklama hatasi: {e}"));
            assert!(t.donme >= 1, "donme sayaci artmali");
            assert_eq!(t.nokta_sayisi(), 1);
        } else {
            panic!("takip baslatilamadi");
        }
    }

    #[test]
    fn sutun_yoksa_hata_verir() {
        let (yol, _g) = kur("t-yok", "a,b\n1,10\n");
        let t = Takip::baslat(&yol, AcSecenekleri::default(), Some("a".into()), "yok", 100);
        assert!(t.is_err());
    }
}
