//! SVG dışa aktarımı.
//!
//! Sorumluluğu `CizimListesi`ni **bağımsız** bir SVG metnine çevirmektir.
//!
//! # Bağımsızlık sözleşmesi (rapor b05, madde 5)
//!
//! Üretilen dosya hiçbir dış kaynağa bağlı **değildir**: `<image>`, `<use>`,
//! dış stil sayfası, betik ve uzak yazı tipi referansı yoktur. Yazı tipi
//! `font-family` olarak **sistem yazı tipi adıyla** verilir (gömülü yazı tipi
//! `@font-face` ile kullanılmaz), çünkü gömülü yazı tipi dosyayı yüzlerce kat
//! büyütür ve lisans sorunu doğurur.
//!
//! # Kaçış
//!
//! Metin içerikleri `&`, `<`, `>`, `"` ve `'` için kaçırılır. Yazı tipi ailesi
//! gibi **sabit** dizeler harf harf yazıldığı için bunlar kaçışsızdır; kaçış
//! yalnızca kullanıcıdan gelen sütun adı / dosya adı gibi serbest metinlere
//! uygulanır.

use std::fmt::Write as _;
use std::path::Path;

use crate::cizim::{CizimListesi, Hiza, Oge};
use crate::hata::{Hata, Sonuc};

/// Gömülü olmayan yazı tipi ailesi adları (sistem yazı tipi referansı).
///
/// SVG `font-family` değeri; tarayıcı/izleyici sisteminde bulunan ilk eşleşmeyi
/// kullanır. Dosya bu yazı tipi dosyalarına **bağlı değildir**.
pub const YAZI_TIPI: &str =
    "ui-sans-serif, system-ui, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif";
/// Sayısal/eksen etiketleri için tek aralıklı yazı tipi ailesi.
pub const YAZI_TIPI_MONO: &str =
    "ui-monospace, 'Cascadia Mono', Consolas, 'DejaVu Sans Mono', Menlo, monospace";

/// SVG kök sürüm ve ad alanı.
const SVG_ACILIS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n";

/// Çizim listesini SVG metnine çevirir.
pub fn svg_metni(l: &CizimListesi) -> String {
    let mut s = String::with_capacity(4096);
    s.push_str(SVG_ACILIS);
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        sayi(l.genislik),
        sayi(l.yukseklik),
        sayi(l.genislik),
        sayi(l.yukseklik)
    );
    // Gömülü stil bloğu: dış stil sayfasına bağımlılık yok.
    let _ = writeln!(s, "<style type=\"text/css\"><![CDATA[");
    let _ = writeln!(
        s,
        "  .p{{fill:none;stroke-linecap:round;stroke-linejoin:round}}"
    );
    let _ = writeln!(s, "  .t{{font-family:{YAZI_TIPI};white-space:pre}}");
    let _ = writeln!(s, "  .m{{font-family:{YAZI_TIPI_MONO};white-space:pre}}");
    let _ = writeln!(s, "]]></style>");
    let _ = writeln!(
        s,
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>",
        sayi(l.genislik),
        sayi(l.yukseklik)
    );

    for oge in &l.ogeler {
        oge_yaz(&mut s, oge);
    }
    s.push_str("</svg>\n");
    s
}

/// SVG'yi yola yazar.
pub fn svg_yaz(l: &CizimListesi, yol: &Path) -> Sonuc<()> {
    use std::io::Write as _;
    let metin = svg_metni(l);
    let mut f = std::fs::File::create(yol).map_err(|kaynak| Hata::CiktiHatasi {
        yol: yol.to_path_buf(),
        kaynak,
    })?;
    f.write_all(metin.as_bytes())
        .map_err(|kaynak| Hata::CiktiHatasi {
            yol: yol.to_path_buf(),
            kaynak,
        })?;
    Ok(())
}

fn oge_yaz(s: &mut String, oge: &Oge) {
    match oge {
        Oge::Izgara {
            x0,
            y0,
            x1,
            y1,
            yatay,
            dikey,
            renk,
        } => {
            for y in yatay {
                let _ = writeln!(
                    s,
                    "<line class=\"p\" x1=\"{}\" x2=\"{}\" y1=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
                    sayi(*x0),
                    sayi(*x1),
                    sayi(*y),
                    sayi(*y),
                    renk.svg_deger()
                );
            }
            for x in dikey {
                let _ = writeln!(
                    s,
                    "<line class=\"p\" y1=\"{}\" y2=\"{}\" x1=\"{}\" x2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
                    sayi(*y0),
                    sayi(*y1),
                    sayi(*x),
                    sayi(*x),
                    renk.svg_deger()
                );
            }
        }
        Oge::Cerceve {
            x0,
            y0,
            x1,
            y1,
            renk,
        } => {
            let _ = writeln!(
                s,
                "<rect class=\"p\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" stroke=\"{}\" stroke-width=\"1\" fill=\"none\"/>",
                sayi(*x0),
                sayi(*y0),
                sayi(x1 - x0),
                sayi(y1 - y0),
                renk.svg_deger()
            );
        }
        Oge::Cizgi {
            noktalar,
            renk,
            kalinlik,
        } => {
            if noktalar.len() < 2 {
                return;
            }
            let yol = noktalar
                .iter()
                .enumerate()
                .map(|(i, (x, y))| {
                    if i == 0 {
                        format!("M{} {}", sayi(*x), sayi(*y))
                    } else {
                        format!("L{} {}", sayi(*x), sayi(*y))
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");
            let _ = writeln!(
                s,
                "<path class=\"p\" d=\"{yol}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                renk.svg_deger(),
                sayi(*kalinlik)
            );
        }
        Oge::Nokta {
            noktalar,
            yaricap,
            renk,
        } => {
            let r = sayi(*yaricap);
            let _ = writeln!(s, "<g fill=\"{}\">", renk.svg_deger());
            for (x, y) in noktalar {
                let _ = writeln!(
                    s,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{r}\"/>",
                    sayi(*x),
                    sayi(*y)
                );
            }
            let _ = writeln!(s, "</g>");
        }
        Oge::Alan {
            noktalar,
            taban,
            saydamlik,
            renk,
            kalinlik,
        } => {
            if noktalar.len() < 2 {
                return;
            }
            let ilk = noktalar[0];
            let son = noktalar[noktalar.len() - 1];
            let yol = noktalar
                .iter()
                .enumerate()
                .map(|(i, (x, y))| {
                    if i == 0 {
                        format!("M{} {}", sayi(*x), sayi(*y))
                    } else {
                        format!("L{} {}", sayi(*x), sayi(*y))
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");
            let yol = format!(
                "{yol} L{} {} L{} {} Z",
                sayi(son.0),
                sayi(*taban),
                sayi(ilk.0),
                sayi(*taban)
            );
            let _ = writeln!(
                s,
                "<path d=\"{yol}\" fill=\"{}\" fill-opacity=\"{}\" class=\"p\" stroke=\"{}\" stroke-width=\"{}\"/>",
                renk.svg_deger(),
                sayi(*saydamlik),
                renk.svg_deger(),
                sayi(*kalinlik)
            );
        }
        Oge::Cubuk { cubuklar, renk } => {
            let _ = writeln!(s, "<g fill=\"{}\">", renk.svg_deger());
            for (x0, y0, x1, y1) in cubuklar {
                let _ = writeln!(
                    s,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    sayi(*x0),
                    sayi(*y0),
                    sayi((x1 - x0).max(0.0)),
                    sayi((y1 - y0).max(0.0))
                );
            }
            let _ = writeln!(s, "</g>");
        }
        Oge::Yatay {
            x0,
            x1,
            y,
            renk,
            kalinlik,
        } => {
            let _ = writeln!(
                s,
                "<line class=\"p\" x1=\"{}\" x2=\"{}\" y1=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                sayi(*x0),
                sayi(*x1),
                sayi(*y),
                sayi(*y),
                renk.svg_deger(),
                sayi(*kalinlik)
            );
        }
        Oge::Dikey {
            x,
            y0,
            y1,
            renk,
            kalinlik,
        } => {
            let _ = writeln!(
                s,
                "<line class=\"p\" y1=\"{}\" y2=\"{}\" x1=\"{}\" x2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                sayi(*y0),
                sayi(*y1),
                sayi(*x),
                sayi(*x),
                renk.svg_deger(),
                sayi(*kalinlik)
            );
        }
        Oge::Metin {
            x,
            y,
            icerik,
            hiza,
            boyut,
            renk,
        } => {
            let anchor = match hiza {
                Hiza::Sol => "start",
                Hiza::Orta => "middle",
                Hiza::Sag => "end",
            };
            let _ = writeln!(
                s,
                "<text class=\"t\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"{anchor}\" fill=\"{}\">{}</text>",
                sayi(*x),
                sayi(*y),
                sayi(*boyut),
                renk.svg_deger(),
                kacis(icerik)
            );
        }
        Oge::GostergeKutu { x, y, kenar, renk } => {
            let _ = writeln!(
                s,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                sayi(*x),
                sayi(*y),
                sayi(*kenar),
                sayi(*kenar),
                renk.svg_deger()
            );
        }
    }
}

/// SVG içeriği için XML kaçışı (`&`, `<`, `>`).
pub fn kacis(s: &str) -> String {
    let mut c = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => c.push_str("&amp;"),
            '<' => c.push_str("&lt;"),
            '>' => c.push_str("&gt;"),
            '"' => c.push_str("&quot;"),
            '\'' => c.push_str("&apos;"),
            _ => c.push(ch),
        }
    }
    c
}

/// SVG'de iki ondalığa kadar yazdırır; tam sayıysa ondalık basmaz.
fn sayi(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let mut s = format!("{v:.2}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cizim::Renk;

    #[test]
    fn xml_kacisi_dogru() {
        assert_eq!(kacis("a&b"), "a&amp;b");
        assert_eq!(kacis("<x>"), "&lt;x&gt;");
        assert_eq!(kacis("\"q\""), "&quot;q&quot;");
    }

    #[test]
    fn svg_kok_ve_dosya_disi_bagimlilik_yok() {
        let l = CizimListesi::yeni(100.0, 80.0);
        let s = svg_metni(&l);
        assert!(s.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(s.contains("</svg>"));
        assert!(!s.contains("xlink:href"), "dış referans olmamalı");
        assert!(!s.contains("@font-face"), "gömülü yazı tipi olmamalı");
    }

    #[test]
    fn sistem_yazi_tipi_adi_kullanilir() {
        let l = CizimListesi::yeni(100.0, 80.0);
        let s = svg_metni(&l);
        assert!(s.contains("font-family:"));
        assert!(s.contains("Segoe UI"));
    }

    #[test]
    fn metin_etiketi_kacirilir() {
        let mut l = CizimListesi::yeni(100.0, 80.0);
        l.ekle(Oge::Metin {
            x: 1.0,
            y: 2.0,
            icerik: "a<b&c".to_string(),
            hiza: Hiza::Sol,
            boyut: 10.0,
            renk: Renk::Metin,
        });
        let s = svg_metni(&l);
        assert!(s.contains("a&lt;b&amp;c"), "kaçış uygulanmalı: {s}");
    }

    #[test]
    fn bos_cizim_listesi_gecerli_svg_uretiir() {
        let l = CizimListesi::yeni(0.0, 0.0);
        let s = svg_metni(&l);
        assert!(s.starts_with("<?xml"));
        assert_eq!(s.matches("<svg").count(), 1);
    }
}
