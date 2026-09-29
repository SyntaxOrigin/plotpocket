//! Terminal (ASCII/Unicode) karakter ızgara çizimi.
//!
//! Sorumluluğu `CizimListesi`ni renksiz bir karakter matrisine rasterlemektir.
//! SVG çıktısıyla **aynı** çizim listesini tüketir; iki ayrı grafik motoru yoktur.
//!
//! # Karakter paleti
//!
//! | Kullanım | Karakter |
//! |---|---|
//! | Veri çizgisi / alan yüksekliği | `▁▂▃▄▅▆▇█` (sekiz seviye) |
//! | Nokta şablonu | `•` |
//! | Izgara / eksen | `·` (soluk) |
//! | Çerçeve | `┌ ─ ┐ │ └ ┘` |
//! | Boş hücre | boşluk |
//!
//! Renk **yoktur**; ızgara yalnızca karakterle kodlanır. Bu, boru hattına uygun
//! ve ekran okuyucularla uyumlu çıktı verir (rapor b05, madde 1).
//!
//! # Ölçek
//!
//! SVG birimleri hücrelere eşlenirken `x` doğrudan ölçeklenir, `y` ise
//! **hücre yüksekliğinin yarısı** kabul edilir: terminal hücreleri yaklaşık
//! 2:1 en/boy oranına sahiptir. Bu düzeltme olmadan tüm grafik tuvalin üst
//! yarısına sıkışır.

use crate::cizim::{CizimListesi, Hiza, Oge, Renk};
use crate::hata::Sonuc;

/// Varsayılan terminal genişliği (sütun).
pub const VARSAYILAN_GENISLIK: usize = 80;
/// Varsayılan terminal yüksekliği (satır).
pub const VARSAYILAN_YUKSEKLIK: usize = 24;
/// Terminal hücresinin en/yükseklik oranı (tipik `2:1`).
pub const HUCRE_ORAN: f64 = 2.0;

/// Yüksekliği temsil eden sekiz blok karakteri.
pub const SEVIYE: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// Izgara karakteri.
pub const IZGARA_KAR: char = '·';
/// Nokta karakteri.
pub const NOKTA_KAR: char = '•';

/// Terminal çıktısı için karakter ızgarası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Izgara {
    /// Sütun sayısı.
    pub genislik: usize,
    /// Satır sayısı.
    pub yukseklik: usize,
    /// Hücreler; `satir * genislik + sutun` indisli.
    pub hucreler: Vec<char>,
    /// Her hücrenin seri rengi (yalnız `debug` çıktısında kullanılır).
    pub renkler: Vec<Renk>,
}

impl Izgara {
    /// Boş ızgara oluşturur.
    pub fn yeni(genislik: usize, yukseklik: usize) -> Self {
        Izgara {
            genislik,
            yukseklik,
            hucreler: vec![' '; genislik * yukseklik],
            renkler: vec![Renk::Izgara; genislik * yukseklik],
        }
    }

    /// Hücreye yazar; koordinat dışarıdaysa sessizce yok sayılır (kırpma).
    pub fn yaz(&mut self, sutun: isize, satir: isize, kar: char, renk: Renk) {
        if sutun < 0 || satir < 0 {
            return;
        }
        let (c, r) = (sutun as usize, satir as usize);
        if c >= self.genislik || r >= self.yukseklik {
            return;
        }
        let i = r * self.genislik + c;
        self.hucreler[i] = kar;
        self.renkler[i] = renk;
    }

    /// Hücreyi okur; dışarıdaysa boşluk döner.
    pub fn oku(&self, sutun: isize, satir: isize) -> char {
        if sutun < 0 || satir < 0 {
            return ' ';
        }
        let (c, r) = (sutun as usize, satir as usize);
        if c >= self.genislik || r >= self.yukseklik {
            return ' ';
        }
        self.hucreler[r * self.genislik + c]
    }

    /// Hücre yüksekliğini 0..1 aralığına çevirir (seviye karakteri için).
    pub fn seviye_kar(&self, sutun: isize, satir: isize) -> Option<char> {
        let c = self.oku(sutun, satir);
        SEVIYE.iter().position(|k| *k == c).map(|i| SEVIYE[i])
    }

    /// Sonuç metnini üretir; satır sonlarında kırpma yapılır.
    pub fn metin(&self) -> String {
        let mut s = String::with_capacity(self.genislik * (self.yukseklik + 1));
        for r in 0..self.yukseklik {
            let bas = r * self.genislik;
            let satir: String = self.hucreler[bas..bas + self.genislik].iter().collect();
            s.push_str(satir.trim_end());
            if r + 1 < self.yukseklik {
                s.push('\n');
            }
        }
        s
    }
}

/// Çizim listesini terminal ızgarasına rasterler.
///
/// # Hatalar
///
/// Genişlik veya yükseklik 0 ise `Hata::GecersizBoyut` döner.
pub fn rasterle(l: &CizimListesi, genislik: usize, yukseklik: usize) -> Sonuc<Izgara> {
    crate::bicim::boyut_dogrula(genislik, yukseklik)?;
    let mut iz = Izgara::yeni(genislik, yukseklik);
    let (ax, ay) = l.alan_baslangic();
    let (bx, by) = l.alan_bitis();
    let g = kenar_yerlesimi(l, genislik, yukseklik, ax, by);

    // SVG birimi -> hücre. **Gövde alanı** (`ax..bx` / `ay..by`) ızgaranın iç
    // kutusuna eşlenir; kenar payları hücre cinsinden ölçülür. Böylece 24
    // satırlık bir terminalde de eksen etiketleri için gerçekten yer kalır
    // (SVG birimi cinsinden ölçmek 24 satırda payların altına düşüyordu).
    //
    // `y` yönü **ters** çevrilir: SVG'de küçük `y` tepe, terminalde küçük satır
    // indeksi tepe olduğu için eşleme doğrudan `y` arttıkça satır artar.
    let gw = (genislik - 1 - g.sol) as f64;
    let gh = ((yukseklik - g.alt) as f64 - 1.0 - g.ust as f64).max(1.0);
    let sol_hucre = g.sol;
    let ust_hucre = g.ust;
    let fw = (bx - ax).max(1.0);
    let fh = (by - ay).max(1.0);
    // Hücre dışına taşan koordinatlar kırpılır. Sınırlama olmazsa çok büyük
    // bir değer `isize` uçlarına dayanır ve `x1 - x0` taşar (panik).
    const SINIR: f64 = 1.0e6;
    let ox = |x: f64| -> isize {
        let v = sol_hucre as f64 + (x - ax) / fw * gw;
        (v.round() as isize).clamp(-SINIR as isize, SINIR as isize)
    };
    let oy = |y: f64| -> isize {
        let v = ust_hucre as f64 + (y - ay) / fh * gh;
        (v.round() as isize).clamp(-SINIR as isize, SINIR as isize)
    };

    let tuval = l.yukseklik;
    let baglam = Baglam {
        ox: &ox,
        oy: &oy,
        yukseklik,
        ay,
        by,
        tuval,
        ax,
    };
    for oge in &l.ogeler {
        oge_ciz(&mut iz, oge, &baglam);
    }
    Ok(iz)
}

/// Izgara kenar payları (hücre cinsinden).
struct Kenar {
    /// Y ekseni etiketleri için ayrılan sütun sayısı.
    sol: usize,
    /// Gövde alanının başladığı satır (üstte ayrılan satır sayısı).
    ust: usize,
    /// Altta ayrılan satır sayısı.
    alt: usize,
}

/// Kenar paylarını çizim listesinden türetir.
///
/// Paylar **gerçek etiket genişliklerinden** hesaplanır, sabit değerden değil:
/// böylece dar terminalde etiket kırpılmaz, geniş terminalde gereksiz yer
/// boş bırakılmaz.
fn kenar_yerlesimi(l: &CizimListesi, genislik: usize, yukseklik: usize, ax: f64, by: f64) -> Kenar {
    let mut sol_etiket = 0usize;
    let mut x_etiket_var = false;
    for oge in &l.ogeler {
        if let Oge::Metin {
            x, y, icerik, hiza, ..
        } = oge
        {
            match hiza {
                Hiza::Sag if *x <= ax => sol_etiket = sol_etiket.max(icerik.chars().count()),
                Hiza::Orta if *y >= by => x_etiket_var = true,
                _ => {}
            }
        }
    }
    // Etiketler için en fazla 12 sütun; ızgaranın çoğunu yemesinler.
    let sol_etiket = sol_etiket.min(12).min(genislik.saturating_sub(4));
    // +1: etiket ile çerçeve arasında boşluk.
    let sol = (sol_etiket + 1).max(2);
    // Satır 0: başlık. Satır 1: y ekseni birimi (başlık varsa).
    let ust = if l.baslik.is_empty() { 1 } else { 2 };
    // x etiketleri, x birimi ve alt bilgi için üç satır.
    let alt = if x_etiket_var { 3 } else { 1 };
    Kenar {
        sol,
        ust,
        alt: alt.min(yukseklik.saturating_sub(ust + 1)),
    }
}

/// `a..=b` aralığını yönü önemsiz hâle getirir (`y` ekseni ters çevrilir).
fn aralik(a: isize, b: isize) -> std::ops::RangeInclusive<isize> {
    if a <= b {
        a..=b
    } else {
        b..=a
    }
}

/// Çok fazla bağımsız parametre taşımamak için raster bağlamı.
struct Baglam<'a> {
    ox: &'a dyn Fn(f64) -> isize,
    oy: &'a dyn Fn(f64) -> isize,
    yukseklik: usize,
    /// Gövde alanı üst `y` (SVG birimi).
    ay: f64,
    /// Gövde alanı alt `y` (SVG birimi).
    by: f64,
    /// Tuvalin SVG yüksekliği.
    tuval: f64,
    /// Gövde alanı sol `x` (SVG birimi).
    #[allow(dead_code)]
    ax: f64,
}

fn oge_ciz(iz: &mut Izgara, oge: &Oge, b: &Baglam<'_>) {
    let (ox, oy) = (b.ox, b.oy);
    match oge {
        Oge::Izgara {
            x0,
            y0,
            x1,
            y1,
            yatay,
            dikey,
            ..
        } => {
            for y in yatay {
                let s = oy(*y);
                for c in aralik(ox(*x0), ox(*x1)) {
                    iz.yaz(c, s, IZGARA_KAR, Renk::Izgara);
                }
            }
            for x in dikey {
                let s = ox(*x);
                for r in aralik(oy(*y0), oy(*y1)) {
                    iz.yaz(s, r, IZGARA_KAR, Renk::Izgara);
                }
            }
        }
        Oge::Cerceve {
            x0,
            y0,
            x1,
            y1,
            renk,
        } => {
            let (c0, c1) = (ox(*x0), ox(*x1));
            let (r0, r1) = (oy(*y0), oy(*y1));
            for c in aralik(c0, c1) {
                iz.yaz(c, r0, '─', *renk);
                iz.yaz(c, r1, '─', *renk);
            }
            for r in aralik(r0, r1) {
                iz.yaz(c0, r, '│', *renk);
                iz.yaz(c1, r, '│', *renk);
            }
            iz.yaz(c0, r0, '┌', *renk);
            iz.yaz(c1, r0, '┐', *renk);
            iz.yaz(c0, r1, '└', *renk);
            iz.yaz(c1, r1, '┘', *renk);
        }
        Oge::Cizgi { noktalar, renk, .. } => {
            polyline_ciz(iz, noktalar, b, *renk);
        }
        Oge::Nokta { noktalar, .. } => {
            for (x, y) in noktalar {
                iz.yaz(ox(*x), oy(*y), NOKTA_KAR, Renk::Bir);
            }
        }
        Oge::Alan {
            noktalar,
            taban,
            renk,
            ..
        } => {
            polyline_ciz(iz, noktalar, b, *renk);
            // Alan: her sutunda veri satirindan tabana kadar dolgu basilir.
            let taban_satir = oy(*taban);
            for w in noktalar.windows(2) {
                dolgu_ciz(iz, w[0], w[1], b, taban_satir);
            }
        }
        Oge::Cubuk { cubuklar, renk } => {
            for (x0, y0, x1, y1) in cubuklar {
                for c in aralik(ox(*x0), ox(*x1)) {
                    for r in aralik(oy(*y0), oy(*y1)) {
                        iz.yaz(c, r, '█', *renk);
                    }
                }
            }
        }
        Oge::Yatay {
            x0, x1, y, renk, ..
        } => {
            let s = oy(*y);
            for c in aralik(ox(*x0), ox(*x1)) {
                iz.yaz(c, s, '─', *renk);
            }
        }
        Oge::Dikey {
            x, y0, y1, renk, ..
        } => {
            let c = ox(*x);
            for r in aralik(oy(*y0), oy(*y1)) {
                iz.yaz(c, r, '│', *renk);
            }
        }
        Oge::Metin {
            x, y, icerik, hiza, ..
        } => {
            let s = metin_satiri(b, *y, *hiza);
            let baslangic = match hiza {
                Hiza::Sol => ox(*x),
                Hiza::Orta => ox(*x) - (icerik.chars().count() as isize) / 2,
                Hiza::Sag => ox(*x) - metin_genislik(icerik) as isize,
            };
            for (i, ch) in icerik.chars().enumerate() {
                iz.yaz(baslangic + i as isize, s, ch, Renk::Metin);
            }
        }
        Oge::GostergeKutu { x, y, kenar, renk } => {
            let (c, r) = (ox(*x), oy(*y));
            let k = ox(*x + *kenar).saturating_sub(c).max(1);
            for i in 0..k {
                iz.yaz(c + i, r, '▪', *renk);
            }
        }
    }
}

/// Metnin hücre cinsinden satır yerleşimi.
///
/// Geometrik öğeler gövde alanına doğrusal eşlenir; **metin** ise hücre
/// tabanlıdır (bir karakter = bir hücre, yazı tipi boyutu yoktur). Bu yüzden
/// metin satırları konumdan değil, etiketin rolünden türetilir:
///
/// | Konum | Satır |
/// |---|---|
/// | Gövde üstü, sola yaslı | 0 (başlık) |
/// | Gövde üstü, sağa yaslı | 1 (y ekseni birimi) |
/// | Gövde altı, ortalanmış, tuvalin dibine yakın | `yukseklik - 3` (x etiketi) |
/// | Gövde altı, ortalanmış | `yukseklik - 2` (x birimi) |
/// | Gövde altı, sağa yaslı | `yukseklik - 1` (alt bilgi) |
/// | Gövde içi | `oy(y)` (y ekseni etiketleri) |
///
/// Gövde altı sınırı `by + 1.0`'dır: en küçük y değeri `by` üzerinde duran y
/// ekseni etiketi **gövde içi** sayılmalıdır; aksi hâlde alt bilgiyle karışır.
fn metin_satiri(b: &Baglam<'_>, y: f64, hiza: Hiza) -> isize {
    if y < b.ay {
        return match hiza {
            Hiza::Sol => 0,
            _ => 1,
        };
    }
    if y > b.by + 1.0 {
        let son = b.yukseklik as isize;
        return match hiza {
            Hiza::Sag => son - 1,
            Hiza::Orta => {
                if y >= b.tuval - 12.0 {
                    son - 2
                } else {
                    son - 3
                }
            }
            Hiza::Sol => son - 3,
        };
    }
    (b.oy)(y)
}

/// Metin karakter sayısı (hizalama kaydırması için).
fn metin_genislik(s: &str) -> usize {
    s.chars().count()
}

/// Polyline karakter ızgarasına çevirir (dikey doldurma tekniği).
///
/// Polyline karakter ızgarasına çevirir (dikey doldurma tekniği).
///
/// Seri ızgaradan geniş olabilir (örn. 2.000 nokta / 64 sütun): bu durumda bir
/// sütuna birden çok örnek düşer. Doğru çizim, her sütunda o sütuna düşen
/// örneklerin **dikey aralığını** boyamaktır; aksi hâlde eğri seyreltik noktalar
/// hâlinde görünür ve ani tepe/çukurlar kaybolur.
///
/// Bu yüzden iki ardışık sütun arasındaki satır farkı boyanır: bu, eğriyi
/// kesintisiz ve **dikey sıçramaları koruyarak** çizer.
fn polyline_ciz(iz: &mut Izgara, noktalar: &[(f64, f64)], b: &Baglam<'_>, renk: Renk) {
    let r_ust = (b.oy)(b.ay);
    let r_alt = (b.oy)(b.by);
    let yuksek = (r_alt - r_ust).max(1) as f64;
    let mut son_sutun: Option<(isize, isize)> = None;
    for w in noktalar.windows(2) {
        let (ax, ayv) = w[0];
        let (bx, byv) = w[1];
        if !ax.is_finite() || !ayv.is_finite() || !bx.is_finite() || !byv.is_finite() {
            continue;
        }
        let (x0, r0) = ((b.ox)(ax), (b.oy)(ayv));
        let (x1, r1) = ((b.ox)(bx), (b.oy)(byv));
        if x1 == x0 {
            // Aynı sütun: iki örnek arası dikey aralık boyanır (ani sıçrama).
            dikey_boyaz(iz, x0, r0, r1, r_ust, yuksek, renk);
            son_sutun = Some((x0, r1));
            continue;
        }
        let adim: isize = if x1 > x0 { 1 } else { -1 };
        let mut s = x0;
        while s != x1 {
            let hedef = s + adim;
            let t2 = (hedef - x0) as f64 / (x1 - x0) as f64;
            let r2 = (r0 as f64 + (r1 - r0) as f64 * t2).round() as isize;
            dikey_boyaz(iz, hedef, r2, r2, r_ust, yuksek, renk);
            // Önceki sütunun satırı ile bu sütunun satırı arası boyanır: eğri
            // böylece kesintisiz çizilir ve dikey sıçramalar korunur.
            let t = (s - x0) as f64 / (x1 - x0) as f64;
            let r1_s = (r0 as f64 + (r1 - r0) as f64 * t).round() as isize;
            dikey_boyaz(iz, hedef, r1_s, r2, r_ust, yuksek, renk);
            s = hedef;
        }
        son_sutun = Some((x1, r1));
    }
    if let Some((x, r)) = son_sutun {
        iz.yaz(x, r, seviye_kar(r, r_ust, yuksek), renk);
    }
}

/// Bir sütunda iki satır arasını blok karakterlerle doldurur.
fn dikey_boyaz(
    iz: &mut Izgara,
    x: isize,
    r0: isize,
    r1: isize,
    r_ust: isize,
    yuksek: f64,
    renk: Renk,
) {
    for r in aralik(r0, r1) {
        iz.yaz(x, r, seviye_kar(r, r_ust, yuksek), renk);
    }
}

fn seviye_kar(satir: isize, r_ust: isize, yuksek: f64) -> char {
    let oran = ((satir - r_ust) as f64 / yuksek).clamp(0.0, 1.0);
    let i = ((1.0 - oran) * 7.0).round() as isize;
    SEVIYE[i.clamp(0, 7) as usize]
}

/// İki nokta arasındaki alanı dikey olarak doldurur (`▒`).
fn dolgu_ciz(iz: &mut Izgara, a: (f64, f64), b_nokta: (f64, f64), b: &Baglam<'_>, taban: isize) {
    if !a.0.is_finite() || !a.1.is_finite() || !b_nokta.0.is_finite() || !b_nokta.1.is_finite() {
        return;
    }
    let (x0, y0) = ((b.ox)(a.0), (b.oy)(a.1));
    let (x1, y1) = ((b.ox)(b_nokta.0), (b.oy)(b_nokta.1));
    for x in aralik(x0, x1) {
        for r in aralik(y0.min(y1), taban) {
            if r < 0 {
                continue;
            }
            // Mevcut hücre veri çizgisiyse korunur, değilse dolgu basılır.
            if iz.hucresi_bos(x, r) {
                iz.yaz(x, r, '▒', Renk::Izgara);
            }
        }
    }
}

impl Izgara {
    /// Hücre boş mu?
    fn hucresi_bos(&self, sutun: isize, satir: isize) -> bool {
        let c = self.oku(sutun, satir);
        c == ' ' || c == IZGARA_KAR
    }
}

/// Çizim listesini doğrudan metne çevirir.
pub fn metin(l: &CizimListesi, genislik: usize, yukseklik: usize) -> Sonuc<String> {
    Ok(rasterle(l, genislik, yukseklik)?.metin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cizim::{Hiza, Renk};
    use crate::terminal::{IZGARA_KAR, SEVIYE};

    fn cizgi_listesi() -> CizimListesi {
        CizimListesi::yeni(200.0, 100.0)
    }

    #[test]
    fn sifir_boyut_hata_verir() {
        let l = cizgi_listesi();
        assert!(rasterle(&l, 0, 10).is_err());
        assert!(rasterle(&l, 10, 0).is_err());
    }

    #[test]
    fn asiri_buyuk_boyut_hata_verir() {
        let l = cizgi_listesi();
        assert!(rasterle(&l, 5000, 10).is_err());
    }

    #[test]
    fn bos_cizim_listesi_bos_izgara_uretiir() {
        let l = cizgi_listesi();
        let iz = rasterle(&l, 40, 10);
        assert!(iz.is_ok());
        if let Ok(iz) = iz {
            assert_eq!(iz.hucreler.len(), 400);
            assert!(iz.hucreler.iter().all(|c| *c == ' '), "bos olmali");
        }
    }

    #[test]
    fn cerceve_kutu_ciziler() {
        let mut l = cizgi_listesi();
        let (ax, ay) = l.alan_baslangic();
        let (bx, by) = l.alan_bitis();
        l.ekle(Oge::Cerceve {
            x0: ax,
            y0: ay,
            x1: bx,
            y1: by,
            renk: Renk::Metin,
        });
        if let Ok(iz) = rasterle(&l, 60, 20) {
            let m = iz.metin();
            assert!(m.contains('┌'), "sol ust koseli olmali:\n{m}");
            assert!(m.contains('└'), "sol alt koseli olmali:\n{m}");
            assert!(m.contains('┐'), "sag ust koseli olmali:\n{m}");
            assert!(m.contains('┘'), "sag alt koseli olmali:\n{m}");
            assert!(m.contains('─'), "yatay cizgi olmali:\n{m}");
            assert!(m.contains('│'), "dikey cizgi olmali:\n{m}");
        } else {
            panic!("rasterlenemedi");
        }
    }

    #[test]
    fn tek_nokta_cizilir() {
        let mut l = cizgi_listesi();
        l.ekle(Oge::Nokta {
            noktalar: vec![(100.0, 50.0)],
            yaricap: 2.0,
            renk: Renk::Bir,
        });
        if let Ok(iz) = rasterle(&l, 60, 20) {
            assert!(iz.metin().contains(NOKTA_KAR), "nokta isareti olmali");
        } else {
            panic!("rasterlenemedi");
        }
    }

    #[test]
    fn iki_noktali_cizgi_cizilir() {
        let mut l = cizgi_listesi();
        // Noktalar gövde alanının **içinde** olmalıdır; dışındaki koordinatlar
        // kırpılır (bu, `cok_genis_aralik_tasma_olmaz` testinin konusudur).
        let (ax, ay) = l.alan_baslangic();
        let (bx, by) = l.alan_bitis();
        let orta = (ay + by) / 2.0;
        l.ekle(Oge::Cizgi {
            noktalar: vec![(ax, orta), (bx, orta)],
            renk: Renk::Bir,
            kalinlik: 1.0,
        });
        if let Ok(iz) = rasterle(&l, 60, 20) {
            // Yatay çizgi tek satırda blok karakteri olarak çizilir; ızgara
            // genişliğinin çoğu boyanmalıdır.
            let dolu = iz.hucreler.iter().filter(|c| SEVIYE.contains(*c)).count();
            assert!(dolu > 40, "yatay cizgi cizilmeli, dolu={dolu}");
        } else {
            panic!("rasterlenemedi");
        }
    }

    #[test]
    fn cok_genis_aralik_tasma_olmaz() {
        let mut l = cizgi_listesi();
        // 1e300 mertebesinde koordinatlar: hiçbir hücreye sığmamalı, panik olmamalı.
        l.ekle(Oge::Cizgi {
            noktalar: vec![(f64::MIN / 2.0, 0.0), (f64::MAX / 2.0, 1e300)],
            renk: Renk::Bir,
            kalinlik: 1.0,
        });
        let r = rasterle(&l, 60, 20);
        assert!(r.is_ok(), "genis aralik panik vermemeli");
    }

    #[test]
    fn sonlu_olmayan_koordinat_cevrilmez() {
        let mut l = cizgi_listesi();
        l.ekle(Oge::Cizgi {
            noktalar: vec![(f64::NAN, f64::NAN), (f64::INFINITY, 1.0)],
            renk: Renk::Bir,
            kalinlik: 1.0,
        });
        assert!(rasterle(&l, 60, 20).is_ok(), "NaN nokta yazilmamali");
    }

    #[test]
    fn izgara_cizgileri_cizilir() {
        let mut l = cizgi_listesi();
        l.ekle(Oge::Izgara {
            x0: 0.0,
            y0: 0.0,
            x1: 200.0,
            y1: 100.0,
            yatay: vec![50.0],
            dikey: vec![100.0],
            renk: Renk::Izgara,
        });
        if let Ok(iz) = rasterle(&l, 60, 20) {
            let sayi = iz.hucreler.iter().filter(|c| **c == IZGARA_KAR).count();
            // Govde alani (0,0)-(200,100) izgaranin tamamina yayilir:
            // 60 sutun yatay + 20 satir dikey - kesisim 1 = 79 hucre.
            assert_eq!(
                sayi, 79,
                "yatay 60 + dikey 20 - kesisim 1 olmali, sayi={sayi}"
            );
        } else {
            panic!("rasterlenemedi");
        }
    }

    #[test]
    fn metin_etiketi_izgaraya_yazilir() {
        let mut l = cizgi_listesi();
        l.ekle(Oge::Metin {
            x: 60.0,
            y: 50.0,
            icerik: "42".to_string(),
            hiza: Hiza::Sag,
            boyut: 10.0,
            renk: Renk::Metin,
        });
        if let Ok(iz) = rasterle(&l, 60, 20) {
            let m = iz.metin();
            assert!(m.contains('4') && m.contains('2'), "etiket yazilmali:\n{m}");
        } else {
            panic!("rasterlenemedi");
        }
    }

    #[test]
    fn seviye_karakterleri_tanimli() {
        assert_eq!(SEVIYE.len(), 8);
        assert_eq!(SEVIYE[0], '▁');
        assert_eq!(SEVIYE[7], '█');
    }

    #[test]
    fn metin_satir_sonunda_kirpilir() {
        let mut iz = Izgara::yeni(10, 2);
        iz.yaz(0, 0, 'X', Renk::Bir);
        let m = iz.metin();
        assert_eq!(
            m, "X\n",
            "basi kirpilmemeli, bos satir sonu kirpilmali: {m:?}"
        );
    }

    #[test]
    fn alan_sablonu_doldurur() {
        let mut l = cizgi_listesi();
        l.ekle(Oge::Alan {
            noktalar: vec![(20.0, 40.0), (100.0, 40.0), (180.0, 40.0)],
            taban: 80.0,
            saydamlik: 0.2,
            renk: Renk::Bir,
            kalinlik: 1.0,
        });
        if let Ok(iz) = rasterle(&l, 60, 20) {
            let dolu = iz.hucreler.iter().filter(|c| **c == '▒').count();
            assert!(dolu > 20, "alan doldurulmali, dolu={dolu}");
        } else {
            panic!("rasterlenemedi");
        }
    }

    #[test]
    fn hucre_disi_konum_yoksayilir() {
        let mut iz = Izgara::yeni(4, 2);
        iz.yaz(100, 100, 'X', Renk::Bir);
        iz.yaz(-1, 0, 'Y', Renk::Bir);
        assert!(iz.hucreler.iter().all(|c| *c == ' '), "disi yazilmamali");
    }

    #[test]
    fn okuma_disi_kutu_bosluk_dondurur() {
        let iz = Izgara::yeni(4, 2);
        assert_eq!(iz.oku(99, 0), ' ');
        assert_eq!(iz.oku(-1, -1), ' ');
    }
}
