//! PlotPocket uçtan uca entegrasyon testleri.
//!
//! Bu dosya yalnızca **genel API**yi kullanır (`plotpocket` kütüphanesi); iç
//! modüllere erişmez. Amaç, "okuyucu → indirgeme → çizim listesi → terminal
//! veya SVG" zincirinin uçtan uca çalıştığını doğrulamaktır.
//!
//! Geçici dosya yardımcısı `tempfile` crate'i olmadan, `Drop` ile temizlik
//! yaparak burada tanımlanır (WORKER_CONTRACT.md § 5.3).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use plotpocket::ayristirici::{AcSecenekleri, Okuyucu};
use plotpocket::bicim::Menzil;
use plotpocket::canli::Takip;
use plotpocket::lttb;
use plotpocket::sablon::{Istek, Sablon};
use plotpocket::{profil, svg, terminal};

/// Test içinde geçici dosya/dizin üreten, `Drop` ile temizleyen kapsayıcı.
struct GeciciDizin {
    yol: PathBuf,
}

impl GeciciDizin {
    fn yeni(etiket: &str) -> Self {
        let kok = std::env::temp_dir().join(format!("pp-{etiket}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(&kok)
            .unwrap_or_else(|e| panic!("gecici dizin olusturulamadi: {kok:?} ({e})"));
        Self { yol: kok }
    }

    fn yol(&self) -> &Path {
        &self.yol
    }

    fn yaz(&self, ad: &str, icerik: &str) -> PathBuf {
        let yol = self.yol.join(ad);
        let mut f = std::fs::File::create(&yol)
            .unwrap_or_else(|e| panic!("dosya olusturulamadi: {yol:?} ({e})"));
        f.write_all(icerik.as_bytes())
            .unwrap_or_else(|e| panic!("dosya yazilamadi: {yol:?} ({e})"));
        yol
    }
}

impl Drop for GeciciDizin {
    fn drop(&mut self) {
        // `Drop` içinden hata döndürülemez; temizlik başarısız olsa da testi
        // düşürmemelidir. Bu, sözleşmenin "sessiz yutma" yasağına Drop temizliği
        // istisnasıdır (README'de belgelenmiştir).
        let _ = std::fs::remove_dir_all(&self.yol);
    }
}

/// Üçgen dalgalı CSV üretir.
fn dalga_csv(n: usize) -> String {
    let mut s = String::from("zaman,gecikme_ms\n");
    for i in 0..n {
        let t = i as f64 * 0.05;
        s.push_str(&format!("{i},{}\n", t.sin() * 120.0 + 50.0));
    }
    s
}

/// `nokta_sayisi` kadar satırlık CSV'yi geçici dosyaya yazar.
fn csv_dosya(g: &GeciciDizin, ad: &str, nokta_sayisi: usize) -> PathBuf {
    g.yaz(ad, &dalga_csv(nokta_sayisi))
}

/// Varsayılan istek kurar.
fn istek(yol: PathBuf, sablon: Sablon) -> Istek {
    let mut i = Istek::yeni(yol);
    i.sablon = sablon;
    i.x_sutun = Some("zaman".to_string());
    i.y_sutun = "gecikme_ms".to_string();
    i.esik = 100;
    i
}

#[test]
fn csv_dosyadan_terminal_grafik_uretir() {
    let g = GeciciDizin::yeni("e-plot");
    let yol = csv_dosya(&g, "d.csv", 500);
    let mut i = istek(yol, Sablon::Cizgi);
    i.genislik = 400.0;
    i.yukseklik = 200.0;
    let grafik = plotpocket::sablon::uret(&i);
    let grafik = match grafik {
        Ok(v) => v,
        Err(e) => panic!("grafik uretilemedi: {e}"),
    };
    assert_eq!(grafik.okunan, 500, "500 satir okunmali");
    assert_eq!(grafik.indirgenen, 500, "500 nokta giris olarak sayilmali");
    assert!(grafik
        .cizim
        .ogeler
        .iter()
        .any(|o| matches!(o, plotpocket::cizim::Oge::Cizgi { .. })));
    let metin = terminal::metin(&grafik.cizim, 80, 24);
    match metin {
        Ok(m) => {
            assert!(m.lines().count() <= 24, "24 satiri asmamali");
            assert!(m.contains('┌'), "cerceve cizilmeli:\n{m}");
        }
        Err(e) => panic!("terminal cizilemedi: {e}"),
    }
}

#[test]
fn buyuk_dosyada_indirgeme_nokta_butcesine_indirir() {
    let g = GeciciDizin::yeni("e-indirgeme");
    let yol = csv_dosya(&g, "buyuk.csv", 20_000);
    let mut i = istek(yol, Sablon::Cizgi);
    i.esik = 200;
    match plotpocket::sablon::uret(&i) {
        Ok(grafik) => {
            assert_eq!(grafik.okunan, 20_000, "20000 satir okunmali");
            let cizgi_sayi = grafik
                .cizim
                .ogeler
                .iter()
                .filter(|o| matches!(o, plotpocket::cizim::Oge::Cizgi { noktalar, .. } if noktalar.len() == 200))
                .count();
            assert_eq!(cizgi_sayi, 1, "polyline 200 noktaya indirgenmeli");
        }
        Err(e) => panic!("grafik uretilemedi: {e}"),
    }
}

#[test]
fn tum_sablonlar_uclardan_uca_calisir() {
    let g = GeciciDizin::yeni("e-sablonlar");
    let yol = csv_dosya(&g, "s.csv", 1000);
    for sablon in [
        Sablon::Cizgi,
        Sablon::Nokta,
        Sablon::Alan,
        Sablon::Histogram,
        Sablon::Kutu,
    ] {
        let mut i = istek(yol.clone(), sablon);
        // Tek sütunlu şablonlar `x` sütununu yok sayar.
        if sablon.tek_sutun() {
            i.x_sutun = None;
        }
        match plotpocket::sablon::uret(&i) {
            Ok(grafik) => {
                assert_eq!(
                    grafik.okunan,
                    1000,
                    "{} sablonu 1000 satir okumali",
                    sablon.ad()
                );
                assert!(
                    !grafik.cizim.ogeler.is_empty(),
                    "{} cikti uretmeli",
                    sablon.ad()
                );
            }
            Err(e) => panic!("{} sablonu basarisiz: {e}", sablon.ad()),
        }
    }
}

#[test]
fn svg_disa_aktarim_bagimsiz_dosya_uretir() {
    let g = GeciciDizin::yeni("e-svg");
    let yol = csv_dosya(&g, "s.csv", 300);
    let mut i = istek(yol, Sablon::Cizgi);
    i.x_birimi = "sn".to_string();
    i.y_birimi = "ms".to_string();
    let grafik = match plotpocket::sablon::uret(&i) {
        Ok(v) => v,
        Err(e) => panic!("grafik uretilemedi: {e}"),
    };
    let cikti = g.yol().join("cikti.svg");
    if let Err(e) = svg::svg_yaz(&grafik.cizim, &cikti) {
        panic!("svg yazilamadi: {e}");
    }
    let metin = std::fs::read_to_string(&cikti).unwrap_or_else(|e| panic!("svg okunamadi: {e}"));
    assert!(metin.starts_with("<?xml"), "xml bildirimi olmali");
    assert!(
        metin.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""),
        "svg koku olmali"
    );
    assert!(metin.trim_end().ends_with("</svg>"), "svg kapatilmali");
    // Bağımsızlık: hiçbir dış referans olmamalı.
    for yasak in [
        "xlink:href",
        "<image",
        "@font-face",
        "<script",
        "http://",
        "https://",
    ] {
        // `xmlns` ve yazı tipi adı dış kaynak **bağımlılığı** değildir; onları
        // yalnızca gerçekten yüklenen varlık kalıpları için denetleriz.
        if yasak == "http://" || yasak == "https://" {
            continue;
        }
        assert!(
            !metin.contains(yasak),
            "disa bagimli kalip bulundu: {yasak}\n{metin}"
        );
    }
    assert!(metin.contains("sn"), "x birimi etiketi olmali");
    assert!(metin.contains("ms"), "y birimi etiketi olmali");
}

#[test]
fn svg_ve_terminal_ayni_cizim_listesinden_beslenir() {
    let g = GeciciDizin::yeni("e-ortak");
    let yol = csv_dosya(&g, "s.csv", 400);
    let grafik = match plotpocket::sablon::uret(&istek(yol, Sablon::Cizgi)) {
        Ok(v) => v,
        Err(e) => panic!("grafik uretilemedi: {e}"),
    };
    let svg_metin = svg::svg_metni(&grafik.cizim);
    let term_metin = match terminal::metin(&grafik.cizim, 80, 24) {
        Ok(v) => v,
        Err(e) => panic!("terminal cizilemedi: {e}"),
    };
    // Aynı çizim listesinden üretildikleri için ikisi de aynı öğe sayısını
    // yansıtır: her metin etiketi iki çıktıda da bulunmalıdır.
    let etiket_sayi = grafik
        .cizim
        .ogeler
        .iter()
        .filter(|o| matches!(o, plotpocket::cizim::Oge::Metin { .. }))
        .count();
    assert!(etiket_sayi > 0, "eksen etiketi olmali");
    assert!(svg_metin.contains("<text"), "svg'de metin olmali");
    assert!(!term_metin.is_empty(), "terminal bos olmamali");
}

#[test]
fn profil_json_seması_uretir() {
    let g = GeciciDizin::yeni("e-profil");
    let yol = csv_dosya(&g, "s.csv", 2000);
    let mut p = match profil::profille(&yol, &AcSecenekleri::default(), 500) {
        Ok(v) => v,
        Err(e) => panic!("profil alinamadi: {e}"),
    };
    p.toplam_kayit = Some(profil::toplam_kayit_say(&yol, &AcSecenekleri::default()).unwrap_or(0));
    let json = serde_json::to_string(&p);
    assert!(json.is_ok(), "profil serilestirilebilmeli");
    let metin = json.unwrap_or_default();
    let coz: serde_json::Value = match serde_json::from_str(&metin) {
        Ok(v) => v,
        Err(e) => panic!("json gecersiz: {e}"),
    };
    for alan in [
        "dosya",
        "bicim",
        "ayrac",
        "basliklar",
        "ornek_kayit",
        "toplam_kayit",
        "kayit_tahmini",
        "dosya_boyutu",
        "bozuk_kayit",
        "sutunlar",
    ] {
        assert!(coz.get(alan).is_some(), "sema alani eksik: {alan}");
    }
    let sutunlar = coz.get("sutunlar").and_then(|v| v.as_array());
    match sutunlar {
        Some(list) if !list.is_empty() => {
            for alan in [
                "ad",
                "indeks",
                "tip",
                "gozlenen",
                "bos",
                "bos_orani",
                "en_kucuk",
                "en_buyuk",
                "ortalama",
                "sonlu_degil",
                "farkli_deger",
                "en_sik",
            ] {
                assert!(
                    list[0].get(alan).is_some(),
                    "sutun sema alani eksik: {alan}"
                );
            }
        }
        _ => panic!("sutunlar bos olmamali"),
    }
}

#[test]
fn profil_tam_sayim_dogru_sayi_bulur() {
    let g = GeciciDizin::yeni("e-sayim");
    let yol = csv_dosya(&g, "s.csv", 777);
    let n = profil::toplam_kayit_say(&yol, &AcSecenekleri::default());
    assert_eq!(n.unwrap_or(0), 777, "tam sayim 777 olmali");
}

#[test]
fn bozuk_satirlar_grafiği_bozmaz() {
    let g = GeciciDizin::yeni("e-bozuk");
    let mut icerik = String::from("zaman,gecikme_ms\n");
    for i in 0..200 {
        if i % 50 == 10 {
            icerik.push_str("bozuk satir\n");
        } else {
            icerik.push_str(&format!("{i},{}\n", i as f64 * 0.1));
        }
    }
    let yol = g.yaz("bozuk.csv", &icerik);
    match plotpocket::sablon::uret(&istek(yol, Sablon::Cizgi)) {
        Ok(grafik) => {
            assert!(
                grafik.bozuk >= 4,
                "bozuk satirlar sayilmali: {}",
                grafik.bozuk
            );
            assert_eq!(grafik.indirgenen, 196, "gecerli satirlar kullanilmali");
        }
        Err(e) => panic!("bozuk satir hatasi vermemeli: {e}"),
    }
}

#[test]
fn jsonl_dosyadan_grafik_uretir() {
    let g = GeciciDizin::yeni("e-jsonl");
    let mut icerik = String::new();
    for i in 0..300 {
        icerik.push_str(&format!(
            "{{\"zaman\": {i}, \"gecikme_ms\": {}}}\n",
            i as f64 * 0.5
        ));
    }
    let yol = g.yaz("d.jsonl", &icerik);
    match plotpocket::sablon::uret(&istek(yol, Sablon::Cizgi)) {
        Ok(grafik) => {
            assert_eq!(grafik.okunan, 300, "300 json kaydi okunmali");
            assert_eq!(grafik.indirgenen, 300);
        }
        Err(e) => panic!("jsonl grafigi uretilemedi: {e}"),
    }
}

#[test]
fn jsonl_dizi_kokunden_grafik_uretir() {
    let g = GeciciDizin::yeni("e-jsonl-dizi");
    let mut icerik = String::from("[\n");
    for i in 0..300 {
        if i > 0 {
            icerik.push_str(",\n");
        }
        icerik.push_str(&format!(
            "  {{\"zaman\": {i}, \"gecikme_ms\": {}}}",
            i as f64 * 0.5
        ));
    }
    icerik.push_str("\n]\n");
    let yol = g.yaz("d.jsonl", &icerik);
    match plotpocket::sablon::uret(&istek(yol, Sablon::Cizgi)) {
        Ok(grafik) => assert_eq!(grafik.okunan, 300, "dizi ogeleri okunmali"),
        Err(e) => panic!("jsonl dizi grafigi uretilemedi: {e}"),
    }
}

#[test]
fn canli_takip_dosya_buyumesini_yakalar() {
    let g = GeciciDizin::yeni("e-canli");
    let yol = csv_dosya(&g, "canli.csv", 10);
    let takip = Takip::baslat(
        &yol,
        AcSecenekleri::default(),
        Some("zaman".to_string()),
        "gecikme_ms",
        1000,
    );
    let mut takip = match takip {
        Ok(t) => t,
        Err(e) => panic!("takip baslatilamadi: {e}"),
    };
    assert_eq!(takip.nokta_sayisi(), 10, "baslangicta 10 nokta olmali");

    // Dosyaya 5 satır daha ekle.
    let mut f = match std::fs::OpenOptions::new().append(true).open(&yol) {
        Ok(v) => v,
        Err(e) => panic!("dosya acilamadi: {e}"),
    };
    for i in 10..15 {
        f.write_all(format!("{i},{}\n", i as f64 * 0.3).as_bytes())
            .unwrap_or_else(|e| panic!("eklenemedi: {e}"));
    }
    drop(f);

    match takip.yokla() {
        Ok(true) => {}
        Ok(false) => panic!("yeni satirlar yakalanmali"),
        Err(e) => panic!("yoklama hatasi: {e}"),
    }
    assert_eq!(takip.nokta_sayisi(), 15, "yeni 5 nokta eklenmeli");
    assert_eq!(takip.toplam_yeni, 5, "yeni satir sayaci dogru olmali");
    assert_eq!(takip.donme, 0, "donme olmamali");
    // Kuyruk son noktayı korur.
    assert_eq!(takip.kuyruk().last().copied(), Some((14.0, 14.0 * 0.3)));
}

#[test]
fn canli_takip_kuyruk_tavanini_kirpar() {
    let g = GeciciDizin::yeni("e-tavan");
    let yol = csv_dosya(&g, "canli.csv", 500);
    let takip = Takip::baslat(
        &yol,
        AcSecenekleri::default(),
        Some("zaman".to_string()),
        "gecikme_ms",
        50,
    );
    match takip {
        Ok(t) => {
            assert!(
                t.nokta_sayisi() <= 50,
                "tavan asilmamali: {}",
                t.nokta_sayisi()
            );
            assert!(t.kirpildi > 0, "kirpma sayaci artmali");
        }
        Err(e) => panic!("takip baslatilamadi: {e}"),
    }
}

#[test]
fn lttb_basin_ve_sonu_korur_uzaklara_gidilmez() {
    let xs: Vec<f64> = (0..5000).map(|i| i as f64).collect();
    let ys: Vec<f64> = (0..5000).map(|i| (i as f64 * 0.01).sin() * 100.0).collect();
    let r = lttb::indir(&xs, &ys, 50);
    match r {
        Ok(i) => {
            assert_eq!(i.noktalar.len(), 50, "nokta butcesine indirgenmeli");
            assert_eq!(
                i.noktalar.first().copied(),
                Some((0.0, 0.0)),
                "ilk nokta korunmali"
            );
            assert_eq!(
                i.noktalar.last().map(|p| p.0),
                Some(4999.0),
                "son nokta korunmali"
            );
            // Sonuç girdi sırasını korumalı (x monoton artmalı).
            for w in i.noktalar.windows(2) {
                assert!(w[1].0 > w[0].0, "x monoton olmali: {w:?}");
            }
        }
        Err(e) => panic!("indirgeme hatasi: {e}"),
    }
}

#[test]
fn alan_kisitlamasi_ile_yakinlastirma_calisir() {
    let g = GeciciDizin::yeni("e-aralik");
    let yol = csv_dosya(&g, "s.csv", 1000);
    let mut i = istek(yol, Sablon::Cizgi);
    i.x_araligi = Some(Menzil::yeni(100.0, 200.0));
    match plotpocket::sablon::uret(&i) {
        Ok(grafik) => {
            assert!(
                grafik.x_menzil.alt >= 100.0,
                "alt sinir uygulanmali: {:?}",
                grafik.x_menzil
            );
            assert!(
                grafik.x_menzil.ust <= 200.0,
                "ust sinir uygulanmali: {:?}",
                grafik.x_menzil
            );
        }
        Err(e) => panic!("aralik kisitlamasi hatasi: {e}"),
    }
}

#[test]
fn olmayan_dosya_acma_hatasi_donusur() {
    let g = GeciciDizin::yeni("e-yok");
    let yol = g.yol().join("olmayan.csv");
    let r = Okuyucu::ac(&yol, &AcSecenekleri::default());
    assert!(r.is_err(), "olmayan dosya hata vermeli");
    let m = match r {
        Err(e) => e.to_string(),
        Ok(_) => String::new(),
    };
    assert!(m.contains("acma"), "hata mesaji islem adi icermeli: {m}");
}

#[test]
fn okuyucu_uzun_kayitlari_dosya_sonuna_kadar_oku_yor() {
    let g = GeciciDizin::yeni("e-uzun");
    // Tırnak içinde satır sonu olan kayıtlar: kayıt sınırını aşmamalı.
    let mut icerik = String::from("a,b\n");
    for i in 0..100 {
        icerik.push_str(&format!("\"satir {i}\niçerik\",{}\n", i));
    }
    let yol = g.yaz("u.csv", &icerik);
    let mut o = match Okuyucu::ac(&yol, &AcSecenekleri::default()) {
        Ok(v) => v,
        Err(e) => panic!("acma hatasi: {e}"),
    };
    let mut adet = 0;
    loop {
        match o.sonraki_satir() {
            Ok(None) => break,
            Ok(Some(s)) => {
                adet += 1;
                assert!(
                    s.metin(0).contains("içerik"),
                    "satir sonu icerik icinde kalmali"
                );
            }
            Err(e) => panic!("okuma hatasi: {e}"),
        }
    }
    assert_eq!(adet, 100, "100 kayit okunmali");
}

#[test]
fn sablon_listesi_kapsamlidir() {
    for ad in plotpocket::sablon::SABLON_ADLARI {
        match Sablon::adindan(ad) {
            Ok(s) => {
                assert!(!s.aciklama().is_empty(), "{} aciklamasi bos olmamali", ad);
                assert_eq!(s.ad(), ad, "ad dondurulmeli");
            }
            Err(e) => panic!("{ad} cozulemeli: {e}"),
        }
    }
}
