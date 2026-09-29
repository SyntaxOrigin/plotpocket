//! Çizim listesi: grafik modelinin tek çıktı biçimi.
//!
//! Raporun en değerli mimari kararı (b07): grafik bir **belge nesnesi değil**,
//! **ekran koordinatlarının bir listesi** olarak tutulur. Her öğe konum, renk ve
//! kalınlık taşır. Aynı liste hem terminal karakter ızgarasına hem de SVG
//! metnine dönüştürülür; iki grafik kütüphanesi gerekmez.
//!
//! Koordinatlar SVG kullanıcı birimi cinsindendir ve **yukarıdan aşağı** akar
//! (SVG'de `y` artar). Terminale çevirirken `cizim.rs` bu yönü tersine çevirir.

use crate::bicim::{tikler, Olcek};
use crate::hata::Sonuc;

/// Palet indeksi. SVG çıktısında sabit bir renge karşılık gelir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renk {
    /// ızgara / eksen çizgileri (soluk gri)
    Izgara,
    /// eksen etiketleri (koyu gri)
    Metin,
    /// birinci seri
    Bir,
    /// ikinci seri
    Iki,
    /// üçüncü seri
    Uc,
    /// dördüncü seri
    Dort,
}

impl Renk {
    /// SVG `stroke` / `fill` değeri.
    pub fn svg_deger(self) -> &'static str {
        match self {
            Renk::Izgara => "#c8d0da",
            Renk::Metin => "#3d4650",
            Renk::Bir => "#1f5fd0",
            Renk::Iki => "#c2410c",
            Renk::Uc => "#0f766e",
            Renk::Dort => "#7c3aed",
        }
    }

    /// Terminalde kullanılan blok karakter seti için indis (`cizgi.rs`).
    pub fn terminal_indis(self) -> usize {
        match self {
            Renk::Izgara => 0,
            Renk::Metin => 1,
            Renk::Bir => 2,
            Renk::Iki => 3,
            Renk::Uc => 4,
            Renk::Dort => 5,
        }
    }
}

/// Metin hizası.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hiza {
    /// Sol kenara yaslanır.
    Sol,
    /// Ortalanır.
    Orta,
    /// Sağ kenara yaslanır.
    Sag,
}

/// Çizim listesinin tek bir öğesi.
#[derive(Debug, Clone, PartialEq)]
pub enum Oge {
    /// Izgara çizgisi: yatay çizgiler `yatay` değerleri, dikey `dikey`.
    Izgara {
        /// Gövde alanı sol `x`.
        x0: f64,
        /// Gövde alanı üst `y`.
        y0: f64,
        /// Gövde alanı sağ `x`.
        x1: f64,
        /// Gövde alanı alt `y`.
        y1: f64,
        /// Yatay çizgilerin `y` koordinatları.
        yatay: Vec<f64>,
        /// Dikey çizgilerin `x` koordinatları.
        dikey: Vec<f64>,
        /// Çizgi rengi.
        renk: Renk,
    },
    /// Çerçeve (eksen kutusu).
    Cerceve {
        /// Sol üst `x`.
        x0: f64,
        /// Sol üst `y`.
        y0: f64,
        /// Sağ alt `x`.
        x1: f64,
        /// Sağ alt `y`.
        y1: f64,
        /// Çizgi rengi.
        renk: Renk,
    },
    /// Polyline (çizgi şablonu).
    Cizgi {
        /// Köşe noktaları.
        noktalar: Vec<(f64, f64)>,
        /// Çizgi rengi.
        renk: Renk,
        /// Çizgi kalınlığı.
        kalinlik: f64,
    },
    /// Noktalar (dağılım şablonu).
    Nokta {
        /// Nokta merkezleri.
        noktalar: Vec<(f64, f64)>,
        /// Nokta yarıçapı.
        yaricap: f64,
        /// Nokta rengi.
        renk: Renk,
    },
    /// Alan (taban seviyesine kadar dolu).
    Alan {
        /// Üst kenar noktaları.
        noktalar: Vec<(f64, f64)>,
        /// Dolgu tabanı `y` koordinatı.
        taban: f64,
        /// Yarı sayı saydamlık (`0.0..1.0`).
        saydamlik: f64,
        /// Kenarlık rengi.
        renk: Renk,
        /// Kenarlık kalınlığı.
        kalinlik: f64,
    },
    /// Dikey çubuk (histogram).
    Cubuk {
        /// Çubukların alt-üst `x` aralıkları ve sol-ust `y`.
        cubuklar: Vec<(f64, f64, f64, f64)>,
        /// Çubuk rengi.
        renk: Renk,
    },
    /// Yatay çizgi (kutu grafiğinde medyan ve kutu kenarları).
    Yatay {
        /// Başlangıç `x`.
        x0: f64,
        /// Bitiş `x`.
        x1: f64,
        /// `y` koordinatı.
        y: f64,
        /// Çizgi rengi.
        renk: Renk,
        /// Kalınlık.
        kalinlik: f64,
    },
    /// Dikey çizgi (kutu grafiğinde kutu gövdesi ve bıyıklar).
    Dikey {
        /// `x` koordinatı.
        x: f64,
        /// Üst `y`.
        y0: f64,
        /// Alt `y`.
        y1: f64,
        /// Çizgi rengi.
        renk: Renk,
        /// Kalınlık.
        kalinlik: f64,
    },
    /// Metin etiketi.
    Metin {
        /// `x` koordinatı.
        x: f64,
        /// `y` koordinatı (taban çizgisi).
        y: f64,
        /// İçerik.
        icerik: String,
        /// Hiza.
        hiza: Hiza,
        /// Yazı tipi boyutu (SVG kullanıcı birimi).
        boyut: f64,
        /// Renk.
        renk: Renk,
    },
    /// Göstergedeki renkli örnek kutu.
    GostergeKutu {
        /// `x`.
        x: f64,
        /// `y`.
        y: f64,
        /// Kenar boyutu.
        kenar: f64,
        /// Renk.
        renk: Renk,
    },
}

/// Hazır çizim listesi.
///
/// Terminal ve SVG çıktısı bu tek yapıdan beslendiği için iki çıktı
/// **matematiksel olarak aynı** görüntüyü üretir.
#[derive(Debug, Clone, PartialEq)]
pub struct CizimListesi {
    /// Tuval genişliği (SVG birimi).
    pub genislik: f64,
    /// Tuval yüksekliği (SVG birimi).
    pub yukseklik: f64,
    /// Başlık metni.
    pub baslik: String,
    /// Alt bilgi (indirgeme bilgisi gibi).
    pub alt_bilgi: String,
    /// Sayısal x ekseni tikleri çizilsin mi.
    ///
    /// `kutu` şablonunda tek eksen vardır; bu şablon için `false` yapılır,
    /// aksi hâlde 0..1 aralığı anlamsız tikler basar.
    pub x_tik_goster: bool,
    /// Gövde öğeleri.
    pub ogeler: Vec<Oge>,
}

impl CizimListesi {
    /// Yeni, boş çizim listesi oluşturur.
    pub fn yeni(genislik: f64, yukseklik: f64) -> Self {
        CizimListesi {
            genislik,
            yukseklik,
            baslik: String::new(),
            alt_bilgi: String::new(),
            x_tik_goster: true,
            ogeler: Vec::new(),
        }
    }

    /// Öğe ekler.
    pub fn ekle(&mut self, oge: Oge) {
        self.ogeler.push(oge);
    }

    /// Gövde alanının sol-üst köşesi (başlık payı).
    pub fn alan_baslangic(&self) -> (f64, f64) {
        (SOL_PAY, UST_PAY)
    }

    /// Gövde alanının sağ-alt köşesi (alt bilgi payı).
    pub fn alan_bitis(&self) -> (f64, f64) {
        (self.genislik - SAG_PAY, self.yukseklik - ALT_PAY)
    }

    /// Başlık ve alt bilgi paylarının dışarıda kalan boyutları.
    pub fn sabit_paylar() -> (f64, f64, f64, f64) {
        (SOL_PAY, UST_PAY, SAG_PAY, ALT_PAY)
    }
}

/// Sol eksen etiketleri için ayrılan pay (birim).
pub const SOL_PAY: f64 = 62.0;
/// Başlık için ayrılan pay.
pub const UST_PAY: f64 = 30.0;
/// Sağ boşluk payı.
pub const SAG_PAY: f64 = 16.0;
/// Alt bilgi ve x ekseni etiketleri için ayrılan pay.
pub const ALT_PAY: f64 = 34.0;

/// Eksen çizgileri, çerçeve ve etiketleri üretir.
///
/// `x_olcek` ve `y_olcek` veri aralığını gövde alanına eşler.
pub fn eksen_koy(
    liste: &mut CizimListesi,
    x_olcek: Olcek,
    y_olcek: Olcek,
    x_birim: &str,
    y_birim: &str,
) -> Sonuc<()> {
    let (ax, ay) = liste.alan_baslangic();
    let (bx, by) = liste.alan_bitis();

    let y_tik = tikler(y_olcek.menzil(), 6);
    let x_tik = if liste.x_tik_goster {
        tikler(x_olcek.menzil(), 8)
    } else {
        Vec::new()
    };
    let yatay: Vec<f64> = y_tik.iter().filter_map(|v| y_olcek.ekrana(*v)).collect();
    let dikey: Vec<f64> = x_tik.iter().filter_map(|v| x_olcek.ekrana(*v)).collect();
    liste.ekle(Oge::Izgara {
        x0: ax,
        y0: ay,
        x1: bx,
        y1: by,
        yatay,
        dikey,
        renk: Renk::Izgara,
    });

    liste.ekle(Oge::Cerceve {
        x0: ax,
        y0: ay,
        x1: bx,
        y1: by,
        renk: Renk::Metin,
    });

    for v in &y_tik {
        if let Some(y) = y_olcek.ekrana(*v) {
            liste.ekle(Oge::Metin {
                x: ax - 6.0,
                y,
                icerik: crate::bicim::sayi_metni(*v),
                hiza: Hiza::Sag,
                boyut: 11.0,
                renk: Renk::Metin,
            });
        }
    }
    for v in &x_tik {
        if let Some(x) = x_olcek.ekrana(*v) {
            liste.ekle(Oge::Metin {
                x,
                y: by + 16.0,
                icerik: crate::bicim::sayi_metni(*v),
                hiza: Hiza::Orta,
                boyut: 11.0,
                renk: Renk::Metin,
            });
        }
    }
    if !x_birim.is_empty() && liste.x_tik_goster {
        liste.ekle(Oge::Metin {
            x: (ax + bx) / 2.0,
            y: liste.yukseklik - 6.0,
            icerik: x_birim.to_string(),
            hiza: Hiza::Orta,
            boyut: 12.0,
            renk: Renk::Metin,
        });
    }
    if !y_birim.is_empty() {
        // Etiket eksen çizgisine sağa yaslıdır; bu sayede terminalde de y
        // etiketleriyle aynı guttered'a (sol sütun) düşer.
        liste.ekle(Oge::Metin {
            x: ax,
            y: ay - 10.0,
            icerik: y_birim.to_string(),
            hiza: Hiza::Sag,
            boyut: 12.0,
            renk: Renk::Metin,
        });
    }
    if !liste.baslik.is_empty() {
        liste.ekle(Oge::Metin {
            x: SOL_PAY,
            y: 18.0,
            icerik: liste.baslik.clone(),
            hiza: Hiza::Sol,
            boyut: 14.0,
            renk: Renk::Metin,
        });
    }
    if !liste.alt_bilgi.is_empty() {
        liste.ekle(Oge::Metin {
            x: liste.genislik - SAG_PAY,
            y: liste.yukseklik - 6.0,
            icerik: liste.alt_bilgi.clone(),
            hiza: Hiza::Sag,
            boyut: 11.0,
            renk: Renk::Metin,
        });
    }
    Ok(())
}
