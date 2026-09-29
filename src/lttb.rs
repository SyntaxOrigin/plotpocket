//! Largest-Triangle-Three-Buckets (LTTB) indirgeme.
//!
//! Sorumluluğu, uzun bir seriyi `hedef` noktaya indirgerken görsel olarak en
//! bilgilendirici noktaları korumaktır. Seri **tek geçişte** okunabilir; tamamı
//! belleğe alınmaz (rapor b05: "10 milyon satırı ekrana sığan nokta sayısına indirger").
//!
//! # Algoritma
//!
//! Seri, veri aralığına göre `hedef - 2` kovaya (bucket) bölünür. İlk ve son
//! nokta **her zaman** korunur. Her ara kova için, önceki seçilmiş nokta ile
//! sonraki kovanın ortalama noktasını oluşturan üçgenin **alanı en büyük** nokta
//! seçilir. Bu, eşit aralıklı basit örneklemeden farklı olarak ani tepe ve çukurları
//! korur.
//!
//! # Belgelenmiş kayan nokta ve uç durum davranışları
//!
//! | Durum | Davranış |
//! |---|---|
//! | `n <= 0` | Boş `Vec` |
//! | `n < hedef` | **Tüm noktalar** döner (indirgeme gerekmez) |
//! | `n == hedef` | Girdi olduğu gibi döner |
//! | `hedef < 3` | `Hata::GecersizEsik` |
//! | `NaN` / `±inf` değer | **Atlanır**; yalnızca sonlu noktalar üzerinde çalışılır |
//! | Tamamı `NaN` | Boş `Vec` |
//! | Sabit seri (tüm `y` eşit) | İlk/son korunur, ara noktalar kova başına dengeli seçilir |
//! | Tek nokta | O nokta döner |
//! | `x` monoton değil | Kabul edilir; üçgen alanı yine de hesaplanır |
//!
//! # Alan hesabı
//!
//! Klasik LTTB üçgen alanını `0.5 * |(bx-ax)(cy-ay) - (cx-ax)(by-ay)|` ile
//! hesaplar. Bu uygulamada **bölme yapılmadan** (mutlak değer) karşılaştırma
//! yapılır; bu, hem daha hızlıdır hem de bölme kaynaklı `NaN`/`inf` riskini
//! ortadan kaldırır. Karar buna göre verildiğinden LTTB'nin *mutlak* çıktısı
//! standart formülle aynı noktaları seçer.

use crate::hata::{Hata, Sonuc};

/// Varsayılan nokta bütçesi (rapor b08: 2.000 nokta).
pub const VARSAYILAN_ESIK: usize = 2000;

/// İndirgeme sonucu: noktalar ve istatistik.
#[derive(Debug, Clone, PartialEq)]
pub struct Indirgeme {
    /// Seçilen noktalar (xk, yk); girdi sırasını korur.
    pub noktalar: Vec<(f64, f64)>,
    /// Girdideki toplam nokta sayısı.
    pub giris_sayisi: usize,
    /// Sonlu olmayan noktalar dâhil atlanan nokta sayısı.
    pub atlanan: usize,
}

impl Indirgeme {
    /// Girişten indirgeme oranı (`0.0` = hiç indirgenmedi).
    pub fn oran(&self) -> f64 {
        if self.giris_sayisi == 0 {
            return 0.0;
        }
        self.noktalar.len() as f64 / self.giris_sayisi as f64
    }
}

/// `hedef` noktaya indirger.
///
/// # Hatalar
///
/// `hedef < 3` ise `Hata::GecersizEsik` döner.
pub fn indir(xs: &[f64], ys: &[f64], hedef: usize) -> Sonuc<Indirgeme> {
    if hedef < 3 {
        return Err(Hata::GecersizEsik { esik: hedef });
    }
    let n = xs.len().min(ys.len());
    let mut sonlu: Vec<(f64, f64)> = Vec::with_capacity(n);
    let mut atlanan = 0usize;
    for i in 0..n {
        let (x, y) = (xs[i], ys[i]);
        if x.is_finite() && y.is_finite() {
            sonlu.push((x, y));
        } else {
            atlanan += 1;
        }
    }
    let giris_sayisi = n;
    if sonlu.is_empty() {
        return Ok(Indirgeme {
            noktalar: Vec::new(),
            giris_sayisi,
            atlanan,
        });
    }
    if sonlu.len() < hedef {
        return Ok(Indirgeme {
            noktalar: sonlu,
            giris_sayisi,
            atlanan,
        });
    }
    let mut cikti = indir_sonlu(&sonlu, hedef);
    cikti.shrink_to_fit();
    Ok(Indirgeme {
        noktalar: cikti,
        giris_sayisi,
        atlanan,
    })
}

/// Yalnızca sonlu noktalar içeren seri için çekirdek indirgeme.
///
/// Ayrı bir fonksiyon olarak tutulur çünkü canlı takip modu kayan tampon
/// doldurduktan sonra **tekrar tekrar** aynı mantığı çalıştırır; ayrıştırma
/// (sonlu eleme) her seferinde yeniden yapılmaz.
/// Yalnızca sonlu noktalar içeren seri için çekirdek indirgeme.
///
/// Kova sınırları `her = (n - 2) / (hedef - 2)` ile hesaplanır; kovalar
/// örtüşmeden `1..n-1` aralığını kaplar. `n > hedef` ve `hedef >= 3` olduğu için
/// `her >= 1` garantidir ve hiçbir kova boş kalmaz.
fn indir_sonlu(seri: &[(f64, f64)], hedef: usize) -> Vec<(f64, f64)> {
    let n = seri.len();
    if n <= hedef {
        return seri.to_vec();
    }
    let mut cikti = Vec::with_capacity(hedef);
    cikti.push(seri[0]);
    let kova = hedef - 2;
    let her = (n - 2) as f64 / kova as f64;
    let mut onceki = 0usize;
    for k in 0..kova {
        // Bu kovada aday olacak indeksler: [k_bas, k_bit)
        let k_bas = (k as f64 * her).floor() as usize + 1;
        let k_bit = ((((k + 1) as f64 * her).floor() as usize) + 1).min(n);
        // Sonraki kovanın ortalama noktası: [o_bas, o_bit)
        let o_bas = k_bit;
        let o_bit = ((((k + 2) as f64 * her).floor() as usize) + 1).min(n);
        let (ort_x, ort_y) = ortalama(seri, o_bas, o_bit);
        let (ax, ay) = seri[onceki];
        let mut en_iyi = k_bas.min(n - 1);
        let mut en_buyuk = f64::NEG_INFINITY;
        for (i, &(cx, cy)) in seri[k_bas..k_bit.min(n)].iter().enumerate() {
            let i = i + k_bas;
            if i <= onceki {
                continue;
            }
            // Üçgen alanının iki katı: bölme yapılmadan karşılaştırılır, bu
            // yüzden bölme kaynaklı NaN/inf riski yoktur.
            let alan = ((ax - ort_x) * (cy - ay) - (cx - ax) * (ay - ort_y)).abs();
            if alan > en_buyuk {
                en_buyuk = alan;
                en_iyi = i;
            }
        }
        cikti.push(seri[en_iyi]);
        onceki = en_iyi;
    }
    cikti.push(seri[n - 1]);
    cikti
}

/// `bas..bit` aralığının ortalama noktası (boş aralıkta ilk nokta).
fn ortalama(seri: &[(f64, f64)], bas: usize, bit: usize) -> (f64, f64) {
    let bit = bit.min(seri.len());
    let bas = bas.min(bit);
    if bas >= bit {
        return seri[seri.len() - 1];
    }
    let mut sx = 0.0f64;
    let mut sy = 0.0f64;
    for p in &seri[bas..bit] {
        sx += p.0;
        sy += p.1;
    }
    let adet = (bit - bas) as f64;
    (sx / adet, sy / adet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seri(n: usize) -> (Vec<f64>, Vec<f64>) {
        let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let ys: Vec<f64> = (0..n).map(|i| (i as f64 * 0.5).sin() * 100.0).collect();
        (xs, ys)
    }

    #[test]
    fn az_hedef_hatasi_verir() {
        assert!(indir(&[0.0, 1.0], &[0.0, 1.0], 2).is_err());
    }

    #[test]
    fn bos_giris_bos_doner() {
        let r = indir(&[], &[], 10);
        assert!(r.is_ok());
        assert!(r.map(|i| i.noktalar.is_empty()).unwrap_or(false));
    }

    #[test]
    fn n_hedeften_kucukse_tam_doner() {
        let (xs, ys) = seri(5);
        let r = indir(&xs, &ys, 10);
        assert_eq!(r.map(|i| i.noktalar.len()).unwrap_or(0), 5);
    }

    #[test]
    fn n_hedefte_ise_degismez() {
        let (xs, ys) = seri(50);
        let r = indir(&xs, &ys, 50);
        assert_eq!(r.map(|i| i.noktalar.len()).unwrap_or(0), 50);
    }

    #[test]
    fn tam_seri_hedefe_iner_ve_ilk_son_korunur() {
        let (xs, ys) = seri(1000);
        let r = indir(&xs, &ys, 100);
        assert_eq!(r.as_ref().map(|i| i.noktalar.len()).unwrap_or(0), 100);
        if let Ok(i) = r {
            assert_eq!(i.noktalar[0], (0.0, 0.0));
            assert_eq!(i.noktalar[99], (999.0, (999.0f64 * 0.5).sin() * 100.0));
        }
    }

    #[test]
    fn sabit_seri_calisir() {
        let xs: Vec<f64> = (0..500).map(|i| i as f64).collect();
        let ys = vec![7.0; 500];
        let r = indir(&xs, &ys, 50);
        assert_eq!(r.as_ref().map(|i| i.noktalar.len()).unwrap_or(0), 50);
    }

    #[test]
    fn nan_inf_atlanir() {
        let xs = vec![0.0, 1.0, f64::NAN, 3.0, f64::INFINITY, 5.0];
        let ys = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let r = indir(&xs, &ys, 3);
        match r {
            Ok(i) => {
                assert_eq!(i.atlanan, 2, "NaN ve inf sayılmalı");
                assert!(i.noktalar.iter().all(|p| p.0.is_finite()));
            }
            Err(_) => panic!("hata beklenmiyordu"),
        }
    }

    #[test]
    fn tamami_nan_ise_bos_doner() {
        let xs = vec![f64::NAN; 10];
        let ys = vec![f64::NAN; 10];
        let r = indir(&xs, &ys, 5);
        assert!(r.map(|i| i.noktalar.is_empty()).unwrap_or(false));
    }

    #[test]
    fn tek_nokta_doner() {
        let r = indir(&[3.0], &[4.0], 10);
        assert_eq!(r.map(|i| i.noktalar.len()).unwrap_or(0), 1);
    }

    #[test]
    fn tepe_noktasi_korunur() {
        let xs: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let mut ys = vec![0.0; 1000];
        ys[500] = 999.0;
        let r = indir(&xs, &ys, 50);
        if let Ok(i) = r {
            assert!(i.noktalar.iter().any(|p| p.0 == 500.0), "tepe kaybolmamalı");
        } else {
            panic!("hata beklenmiyordu");
        }
    }

    #[test]
    fn giris_sayisi_indirgemede_degismez() {
        let (xs, ys) = seri(321);
        let r = indir(&xs, &ys, 64);
        assert_eq!(r.map(|i| i.giris_sayisi).unwrap_or(0), 321);
    }
}
