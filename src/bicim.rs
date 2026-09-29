//! Eksen ölçekleme, "güzel" tik seçimi ve sayı biçimlendirme.
//!
//! Sorumluluğu veri aralığını ekran aralığına **doğrusal** eşlemektir ve
//! eksen etiketlerini okunur biçimde üretmektir. Veri, dosya veya çizim bilmez.
//!
//! # Belgelenmiş uç durum davranışları
//!
//! | Durum | Davranış |
//! |---|---|
//! | `alt == ust` (sabit seri) | Aralık `±1` (veya `±|v|·0.05` sıfırdan büyükse) ile açılır |
//! | Aralık tamamen negatif | Alt/üst yer değiştirilir, işaret korunur |
//! | Değer sonlu değil (`NaN`/`inf`) | Ölçek hesabında yok sayılır, nokta çizilmez |
//! | `NaN` tüm veri | Ölçek `0..1` olur, grafik boş çizilir, hata **verilmez** |
//! | Çok büyük/küçük (`1e308`, `1e-308`) | Tik üretimi üstel adım kullanır, taşma yok |
//! | Genişlik/yükseklik 0 | `Hata::GecersizBoyut` |
//!
//! "Güzel" tik adımı `1 / 2 / 5 × 10^k` ailesinden seçilir; bu, ızgarada tam
//! sayı etiketleri verir.

use crate::hata::{Hata, Sonuc};

/// Veri aralığı (alt/üst sınır).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Menzil {
    /// Alt sınır.
    pub alt: f64,
    /// Üst sınır.
    pub ust: f64,
}

impl Menzil {
    /// Verilen alt/üst sınırdan menzil oluşturur; `alt > ust` ise düzeltir.
    pub fn yeni(alt: f64, ust: f64) -> Menzil {
        if alt.is_nan() || ust.is_nan() {
            return Menzil { alt: 0.0, ust: 1.0 };
        }
        if alt > ust {
            return Menzil { alt: ust, ust: alt };
        }
        Menzil { alt, ust }
    }

    /// Verilen noktalardan **tek eksenin** menzilini üretir; sonlu olmayanlar
    /// atlanır, veri yoksa `0..1`.
    ///
    /// `noktalardan` her iki ekseni birleştirdiği için yalnızca tek sütunlu
    /// şablonlarda (histogram, kutu) kullanılmalıdır; x/y ayrı eksenlerde
    /// **bu** fonksiyon kullanılır.
    pub fn degerlerden(d: &[f64]) -> Menzil {
        let mut alt = f64::INFINITY;
        let mut ust = f64::NEG_INFINITY;
        for v in d {
            if v.is_finite() {
                alt = alt.min(*v);
                ust = ust.max(*v);
            }
        }
        if !alt.is_finite() || !ust.is_finite() {
            return Menzil { alt: 0.0, ust: 1.0 };
        }
        Menzil::yeni(alt, ust)
    }

    /// Verilen noktalardan menzil; sonlu olmayanlar atlanır, veri yoksa `0..1`.
    ///
    /// x ve y **tek** menzilde birleştirilir. Bu, bir eksende gösterilen
    /// tek sütunlu seriler (histogram, kutu) için doğrudur; iki eksenli
    /// grafiklerde `degerlerden` kullanılmalıdır.
    pub fn noktalardan(xs: &[f64], ys: &[f64]) -> Menzil {
        let n = xs.len().min(ys.len());
        let mut alt = f64::INFINITY;
        let mut ust = f64::NEG_INFINITY;
        for i in 0..n {
            let (x, y) = (xs[i], ys[i]);
            if x.is_finite() {
                alt = alt.min(x);
                ust = ust.max(x);
            }
            if y.is_finite() {
                alt = alt.min(y);
                ust = ust.max(y);
            }
        }
        if !alt.is_finite() || !ust.is_finite() {
            return Menzil { alt: 0.0, ust: 1.0 };
        }
        Menzil::yeni(alt, ust)
    }

    /// Menzilin genişliği; sıfır aralıkta `0` döner.
    pub fn genislik(&self) -> f64 {
        self.ust - self.alt
    }

    /// Sıfır aralığı makul bir genişliğe açar (grafik çizilemezdi).
    pub fn genislet(&self) -> Menzil {
        if self.ust > self.alt {
            return *self;
        }
        let merkez = self.alt;
        let yari = if merkez.abs() > 0.0 {
            merkez.abs() * 0.05
        } else {
            1.0
        };
        Menzil {
            alt: merkez - yari,
            ust: merkez + yari,
        }
    }

    /// Bu menzili `araliga` sınırlar; kısıtlama boşsa menzil aynen kalır.
    pub fn kisitla(&self, araliga: Option<Menzil>) -> Menzil {
        match araliga {
            None => *self,
            Some(a) => Menzil::yeni(self.alt.max(a.alt), self.ust.min(a.ust)),
        }
    }
}

/// Doğrusal ölçek: veri değeri → ekran koordinatı.
#[derive(Debug, Clone, Copy)]
pub struct Olcek {
    veri: Menzil,
    ekran_bas: f64,
    ekran_bit: f64,
}

impl Olcek {
    /// Veri menzilini `[ekran_bas, ekran_bit]` aralığına eşler.
    ///
    /// Sıfır genişlikli veri menzili `genislet()` ile düzeltilir, böylece
    /// bölme sıfıra düşmez ve çizim `NaN` üretmez.
    pub fn yeni(veri: Menzil, ekran_bas: f64, ekran_bit: f64) -> Olcek {
        let veri = veri.genislet();
        Olcek {
            veri,
            ekran_bas,
            ekran_bit,
        }
    }

    /// Veri değerini ekran koordinatına eşler; sonlu olmayanlar `None`.
    pub fn ekrana(&self, deger: f64) -> Option<f64> {
        if !deger.is_finite() {
            return None;
        }
        let oran = (deger - self.veri.alt) / self.veri.genislik();
        Some(self.ekran_bas + oran * (self.ekran_bit - self.ekran_bas))
    }

    /// Ekran koordinatını veri değerine eşler (ters işlem).
    pub fn veriye(&self, ekran: f64) -> f64 {
        let g = self.ekran_bit - self.ekran_bas;
        if g == 0.0 {
            return self.veri.alt;
        }
        let oran = (ekran - self.ekran_bas) / g;
        self.veri.alt + oran * self.veri.genislik()
    }

    /// Ölçeğin veri menzili.
    pub fn menzil(&self) -> Menzil {
        self.veri
    }
}

/// Verilen menzile en az `adet` tane "güzel" tik üretir.
pub fn tikler(menzil: Menzil, adet: usize) -> Vec<f64> {
    let m = menzil.genislet();
    if adet == 0 || !m.genislik().is_finite() {
        return vec![m.alt, m.ust];
    }
    let ham = m.genislik() / adet as f64;
    let adim = guzel_adim(ham);
    if !adim.is_finite() || adim <= 0.0 {
        return vec![m.alt, m.ust];
    }
    let ilk = (m.alt / adim).ceil() * adim;
    let son = m.ust;
    // Taşma ve sonsuz döngü koruması: en fazla `adet * 4 + 8` tik üretilir.
    let tav = adet.saturating_mul(4).saturating_add(8);
    let mut liste = Vec::with_capacity(tav);
    let mut v = ilk;
    while v <= son && liste.len() < tav {
        if v.is_finite() {
            liste.push(v);
        }
        v += adim;
    }
    if liste.is_empty() {
        liste.push(m.alt);
        liste.push(m.ust);
    }
    liste
}

/// `1 / 2 / 5 × 10^k` ailesinden, `ham`'a en yakın ve `ham`'dan büyük adım.
pub fn guzel_adim(ham: f64) -> f64 {
    if !ham.is_finite() || ham <= 0.0 {
        return 1.0;
    }
    let k = ham.log10().floor();
    let us = 10f64.powf(k);
    if !us.is_finite() || us == 0.0 {
        return ham;
    }
    let m = ham / us;
    let carpan = if m <= 1.0 {
        1.0
    } else if m <= 2.0 {
        2.0
    } else if m <= 5.0 {
        5.0
    } else {
        10.0
    };
    carpan * us
}

/// Değeri eksen etiketi olarak kısa biçimde yazar.
///
/// `NaN` → `NaN`, `±inf` → `±∞`, `|v| ≥ 1e6` veya `0 < |v| < 1e-4` ise üstel
/// gösterim, diğer hâllerde en fazla 6 ondalık ve kırpılmış sıfırlar.
pub fn sayi_metni(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "∞".to_string()
        } else {
            "-∞".to_string()
        };
    }
    let a = v.abs();
    if a != 0.0 && (a >= 1e6 || a < 1e-4) {
        // `1.500e6` yerine `1.5e6`: tek ondalık yeterli, kalan sıfırlar atılır.
        return format!("{v:.1e}").replace("e0", "e").replace("e-0", "e-");
    }
    let mut s = format!("{v:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Genişlik ve yüksekliği doğrular (1..=2000).
pub fn boyut_dogrula(genislik: usize, yukseklik: usize) -> Sonuc<()> {
    if genislik == 0 || yukseklik == 0 || genislik > 2000 || yukseklik > 2000 {
        return Err(Hata::GecersizBoyut {
            genislik,
            yukseklik,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sifir_aralik_genisletilir() {
        let m = Menzil::yeni(5.0, 5.0).genislet();
        assert!(m.ust > m.alt, "sabit seri genişlemeli: {m:?}");
    }

    #[test]
    fn negatif_ter_dondurulur_duzeltilir() {
        let m = Menzil::yeni(10.0, -10.0);
        assert_eq!(m.alt, -10.0);
        assert_eq!(m.ust, 10.0);
    }

    #[test]
    fn nan_menzil_sifir_bir_araliga_duser() {
        let m = Menzil::yeni(f64::NAN, f64::NAN);
        assert_eq!(m.alt, 0.0);
        assert_eq!(m.ust, 1.0);
    }

    #[test]
    fn olce_dogrusal_esler() {
        let o = Olcek::yeni(Menzil::yeni(0.0, 10.0), 0.0, 100.0);
        assert!((o.ekrana(5.0).unwrap_or(-1.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn sonlu_olmayan_nokta_cizilmez() {
        let o = Olcek::yeni(Menzil::yeni(0.0, 1.0), 0.0, 10.0);
        assert!(o.ekrana(f64::NAN).is_none());
        assert!(o.ekrana(f64::INFINITY).is_none());
    }

    #[test]
    fn cok_buyuk_deger_tasmaz() {
        let o = Olcek::yeni(Menzil::yeni(0.0, 1e308), 0.0, 80.0);
        let y = o.ekrana(5e307);
        assert!(
            y.map(|v| v.is_finite()).unwrap_or(false),
            "sonlu olmalı: {y:?}"
        );
    }

    #[test]
    fn nan_tek_veri_noktasi_boz_aralik_verir() {
        let m = Menzil::noktalardan(&[f64::NAN], &[f64::NAN]);
        assert_eq!(m.alt, 0.0);
        assert_eq!(m.ust, 1.0);
    }

    #[test]
    fn guzel_adim_bes_kuvveti_secir() {
        assert_eq!(guzel_adim(1.0), 1.0);
        assert_eq!(guzel_adim(3.0), 5.0);
        assert_eq!(guzel_adim(7.0), 10.0);
        assert_eq!(guzel_adim(0.03), 0.05);
    }

    #[test]
    fn tikler_menzil_ici_kalir() {
        let t = tikler(Menzil::yeni(0.0, 100.0), 5);
        assert!(t.len() >= 4, "yeterli tik: {t:?}");
        assert!(
            t.iter().all(|v| *v >= 0.0 && *v <= 100.0),
            "tikler menzil dışına çıkmamalı: {t:?}"
        );
    }

    #[test]
    fn sayi_metni_kisa_ve_dogru() {
        assert_eq!(sayi_metni(1.5), "1.5");
        assert_eq!(sayi_metni(-0.0), "0");
        assert_eq!(sayi_metni(f64::NAN), "NaN");
        assert_eq!(sayi_metni(f64::INFINITY), "∞");
        assert!(sayi_metni(1e7).contains('e'), "üstel: {}", sayi_metni(1e7));
    }
}
