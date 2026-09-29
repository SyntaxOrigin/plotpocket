//! CSV / TSV / JSONL okuyucu, sütun tipi çıkarımı ve kayan tamponlu kayıt tarayıcı.
//!
//! Sorumluluğu dosya baytlarını **kayıt kayıt** üretmek ve her alanı
//! `Bos` / `Sayi` / metin olarak sınıflandırmaktır. Grafik, ölçekleme ve çizim
//! bilmez; yalnızca sütun adlarını ve alan değerlerini bilir.
//!
//! # Bellek sözleşmesi
//!
//! Dosya **hiçbir zaman** `read_to_string` ile belleğe alınmaz. Okuyucu sabit
//! boyutlu bir `BufReader` üzerinden ilerler; yalnızca **geçerli kayıt** tutulur.
//! Tırnak içinde satır sonu varsa kayıt büyüyebilir, üst sınır `MAKS_KAYIT_BAYTI`
//! ile korunur. Biçim algılama için okunan önek `Cursor` ile akışın **başına**
//! geri sarılır, böylece dosya ikinci kez açılmaz.
//!
//! # CSV leksikası
//!
//! - Alan ayracı dosya adından veya içerikten (en sık sayılan ayraç) seçilir.
//! - `"` ile tırnaklanan alan: içindeki ayraç ve satır sonu **literal** sayılır.
//! - `""` kaçışlı tırnak → tek `"` karakteri; `\"` de kabul edilir (gevşek kaçış).
//! - Tırnak içindeki satır sonu kaydı bölemez.
//! - Alan sayısı başlıkla uyuşmayan kayıt **bozuk** sayılır, atlanır ve sayacı artar.
//!
//! # JSONL leksikası
//!
//! İki kök desteklenir: satır başına bir değer (NDJSON) ve kök `[]` dizisi
//! (biçimlendirilmiş JSON dizisi). Diziler kök için açıldığında öğeler virgülle
//! ayrılmış olarak akış hâlinde üretilir. Sütun adları ilk kaydın anahtarlarından
//! alınır; `serde_json::Map` sözlük olduğu için anahtar sırası **sabit ve
//! alfabetiktir**. Sonraki kayıtlarda görülen **yeni** anahtarlar sona eklenir.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

use crate::hata::{Hata, Sonuc};

/// Tek bir kaydın bayt uzunluğu için üst sınır (8 MiB).
///
/// Tırnak dengesi bozuk bir dosyada `read_until` sınırsız büyüyebilir; bu sınır
/// bellek tüketimini engeller. Aşan kayıt bozuk sayılır ve bir sonraki satır
/// sonuna kadar atılır.
pub const MAKS_KAYIT_BAYTI: usize = 8 * 1024 * 1024;

/// Biçim algılama ve ayraç sayımı için okunan önek boyutu (64 KiB).
pub const ONEK_BAYTI: usize = 64 * 1024;

/// Bir kaydın tek bir alanı.
#[derive(Debug, Clone, PartialEq)]
pub enum Alan {
    /// Alan yok (kısa kayıt) veya boş metin.
    Bos,
    /// Alan sayısal olarak ayrıştırıldı (`NaN` / `inf` dâhil).
    Sayi(f64),
    /// Metin; kayıt tamponunda `[bas, bit)` bayt aralığı.
    Duz {
        /// Başlangıç ofseti.
        bas: u32,
        /// Bitiş ofseti (hariç).
        bit: u32,
    },
    /// Metin; kaçış çözüldüğü için açılan tamponda `[bas, bit)` bayt aralığı.
    Acik {
        /// Başlangıç ofseti.
        bas: u32,
        /// Bitiş ofseti (hariç).
        bit: u32,
    },
}

impl Alan {
    /// Alanın sayısal değeri; metin veya boş alan için `None`.
    pub fn sayi(&self) -> Option<f64> {
        match self {
            Alan::Sayi(v) => Some(*v),
            _ => None,
        }
    }

    /// Alanın metin değeri. Sayısal alanlar `sayi_metni` çıktısını verir.
    pub fn metin(&self, tampon: &str, acilan: &str) -> String {
        match self {
            Alan::Bos => String::new(),
            Alan::Sayi(v) => crate::bicim::sayi_metni(*v),
            Alan::Duz { bas, bit } => dilim(tampon, *bas, *bit).to_string(),
            Alan::Acik { bas, bit } => dilim(acilan, *bas, *bit).to_string(),
        }
    }
}

fn dilim(s: &str, bas: u32, bit: u32) -> &str {
    let b = bas as usize;
    let t = bit as usize;
    if b <= t && t <= s.len() && s.is_char_boundary(b) && s.is_char_boundary(t) {
        &s[b..t]
    } else {
        ""
    }
}

/// Okunan tek kayıt.
#[derive(Debug, Clone, Copy)]
pub struct Satir<'a> {
    /// Kaydın 1 tabanlı numarası.
    pub numara: u64,
    /// Kaydın alanları.
    pub alanlar: &'a [Alan],
    /// Kaydın ham metni.
    tampon: &'a str,
    /// Kaçış çözülmüş alanların biriktiği tampon.
    acilan: &'a str,
}

impl<'a> Satir<'a> {
    /// Verilen indeksdeki alanın metni; indeks yoksa boş dize.
    pub fn metin(&self, indeks: usize) -> String {
        match self.alanlar.get(indeks) {
            Some(a) => a.metin(self.tampon, self.acilan),
            None => String::new(),
        }
    }

    /// Verilen indeksdeki alanın sayısal değeri.
    pub fn sayi(&self, indeks: usize) -> Option<f64> {
        self.alanlar.get(indeks).and_then(Alan::sayi)
    }

    /// Kayıttaki alan sayısı.
    pub fn alan_sayisi(&self) -> usize {
        self.alanlar.len()
    }
}

/// Tanınan dosya biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bicim {
    /// Virgülle ayrılmış metin.
    Csv,
    /// Sekme ile ayrılmış metin.
    Tsv,
    /// Satır başına bir JSON değeri (NDJSON).
    JsonlSatir,
    /// Kök `[]` dizisi olan JSON.
    JsonlDizi,
}

impl Bicim {
    /// Biçimin `profile` çıktısında gösterilecek adı.
    pub fn ad(self) -> &'static str {
        match self {
            Bicim::Csv => "csv",
            Bicim::Tsv => "tsv",
            Bicim::JsonlSatir => "jsonl-satir",
            Bicim::JsonlDizi => "jsonl-dizi",
        }
    }
}

/// `Okuyucu::ac` için kullanıcı ayarları.
#[derive(Debug, Clone)]
pub struct AcSecenekleri {
    /// Elle verilen alan ayracı (bayt). `None` ise içerikten çıkarılır.
    pub ayrac: Option<u8>,
    /// `false` ise ilk kayıt başlık sayılmaz.
    pub baslik: bool,
}

impl Default for AcSecenekleri {
    /// Varsayılan: ayraç içerikten çıkarılsın, ilk kayıt başlık olsun.
    fn default() -> Self {
        Self {
            ayrac: None,
            baslik: true,
        }
    }
}

/// CSV / TSV / JSONL okuyucu.
pub struct Okuyucu {
    girdi: Box<dyn BufRead + Send>,
    yol: PathBuf,
    bicim: Bicim,
    ayrac: u8,
    basliklar: Vec<String>,
    sutun_indeks: HashMap<String, usize>,
    tampon: String,
    acilan: String,
    ham: Vec<u8>,
    alanlar: Vec<Alan>,
    satir_no: u64,
    bozuk_satir: u64,
    json_akisi: JsonAkisi,
    json_bitti: bool,
    /// Başlıksız dosyada ilk kayıt hâlâ teslim edilmemiş.
    ilk_kayit_bekliyor: bool,
}

impl Okuyucu {
    /// Dosyayı açar, biçimi ve başlıkları belirler.
    ///
    /// # Hatalar
    ///
    /// Dosya açılamaz/okunamaz ise `Hata::Io`, biçim belirlenemezse
    /// `Hata::BilinmeyenBicim` döner.
    pub fn ac(yol: &Path, secenek: &AcSecenekleri) -> Sonuc<Self> {
        let dosya = File::open(yol).map_err(|kaynak| Hata::Io {
            islem: "acma",
            yol: yol.to_path_buf(),
            kaynak,
        })?;
        let onek = oku_onek(&dosya).map_err(|kaynak| Hata::Io {
            islem: "okuma",
            yol: yol.to_path_buf(),
            kaynak,
        })?;
        let (bicim, ayrac) = bicim_ve_ayrac(yol, &onek, secenek)?;
        let json_dizi = bicim == Bicim::JsonlDizi;

        // Önek akışın başına eklenir: `Cursor::chain(BufReader)` sırayı korur.
        let girdi: Box<dyn BufRead + Send> =
            Box::new(Cursor::new(onek).chain(BufReader::with_capacity(64 * 1024, dosya)));

        let mut okuyucu = Okuyucu {
            girdi,
            yol: yol.to_path_buf(),
            bicim,
            ayrac,
            basliklar: Vec::new(),
            sutun_indeks: HashMap::new(),
            tampon: String::new(),
            acilan: String::new(),
            ham: Vec::with_capacity(8192),
            alanlar: Vec::new(),
            satir_no: 0,
            bozuk_satir: 0,
            json_akisi: JsonAkisi::yeni(json_dizi),
            json_bitti: false,
            ilk_kayit_bekliyor: false,
        };
        okuyucu.basligi_oku(secenek.baslik)?;
        Ok(okuyucu)
    }

    /// Tanınan biçim.
    pub fn bicim(&self) -> Bicim {
        self.bicim
    }

    /// Kullanılan alan ayracı (JSONL için `0`).
    pub fn ayrac(&self) -> u8 {
        self.ayrac
    }

    /// Başlık adları.
    pub fn basliklar(&self) -> &[String] {
        &self.basliklar
    }

    /// Toplam sütun sayısı.
    pub fn sutun_sayisi(&self) -> usize {
        self.basliklar.len()
    }

    /// Ada göre sütun indeksi.
    pub fn sutun_indeksi(&self, ad: &str) -> Option<usize> {
        self.sutun_indeks.get(ad).copied()
    }

    /// Şu ana kadar atlanan bozuk kayıt sayısı.
    pub fn bozuk_satir_sayisi(&self) -> u64 {
        self.bozuk_satir
    }

    /// Okunan son kayıt numarası.
    pub fn satir_no(&self) -> u64 {
        self.satir_no
    }

    /// Bir sonraki geçerli kaydı okur.
    ///
    /// Bozuk kayıtlar atlanır ve `bozuk_satir_sayisi()` artar; dosya sonunda
    /// `None` döner. Böylece canlı takipte tek bozuk satır grafiği öldürmez
    /// (rapor b07, "Hata yönetimi").
    pub fn sonraki_satir(&mut self) -> Sonuc<Option<Satir<'_>>> {
        loop {
            if self.json_bitti {
                return Ok(None);
            }
            if self.ilk_kayit_bekliyor {
                self.ilk_kayit_bekliyor = false;
                if self.alanlar.len() == self.basliklar.len() {
                    return Ok(Some(Satir {
                        numara: self.satir_no,
                        alanlar: &self.alanlar,
                        tampon: &self.tampon,
                        acilan: &self.acilan,
                    }));
                }
                self.satir_no += 1;
                self.bozuk_satir += 1;
            } else if matches!(self.bicim, Bicim::JsonlSatir | Bicim::JsonlDizi) {
                if !self.json_kaydi_oku()? {
                    return Ok(None);
                }
            } else if !self.csv_kaydi_oku()? {
                return Ok(None);
            }
            if self.alanlar.len() == self.basliklar.len() {
                return Ok(Some(Satir {
                    numara: self.satir_no,
                    alanlar: &self.alanlar,
                    tampon: &self.tampon,
                    acilan: &self.acilan,
                }));
            }
            self.satir_no += 1;
            self.bozuk_satir += 1;
        }
    }

    /// Başlık satırını okur; `baslik` yoksa `s1..sn` üretilir.
    ///
    /// Başlık yoksa ilk kayıt **veri olarak korunur** ve `s1..sn` adları ondan
    /// türetilir; bu kayıt bir sonraki `sonraki_satir` çağrısında döner.
    ///
    /// JSONL'de başlık satırı yoktur: sütun adları **ilk kaydın anahtarlarından**
    /// türetilir. Bu yüzden JSONL'de de ilk kayıt önceden okunur; aksi hâlde
    /// `Okuyucu::ac` çağıranı sıfır sütunla baş başa bırakırdı.
    fn basligi_oku(&mut self, baslik_var: bool) -> Sonuc<()> {
        if matches!(self.bicim, Bicim::JsonlSatir | Bicim::JsonlDizi) {
            if self.json_kaydi_oku()? {
                self.ilk_kayit_bekliyor = true;
            } else {
                self.sutun_ad_ekle("deger".to_string());
            }
            return Ok(());
        }
        if baslik_var {
            if !self.csv_kaydi_oku()? {
                // Boş dosya: tek sahte sütunla devam et; çağıran "sütun yok"
                // hatasını alır.
                self.sutun_ad_ekle("s1".to_string());
                return Ok(());
            }
            let n = self.alanlar.len();
            for i in 0..n {
                let ham = self.alanlar[i].metin(&self.tampon, &self.acilan);
                let ad = if ham.trim().is_empty() {
                    format!("s{}", i + 1)
                } else {
                    ham.trim().to_string()
                };
                self.sutun_ad_ekle(ad);
            }
            return Ok(());
        }
        // Başlıksız: ilk kayıttan sütun adları türet, kaydı geri ver.
        if !self.csv_kaydi_oku()? {
            self.sutun_ad_ekle("s1".to_string());
            return Ok(());
        }
        for i in 0..self.alanlar.len() {
            self.sutun_ad_ekle(format!("s{}", i + 1));
        }
        self.ilk_kayit_bekliyor = true;
        Ok(())
    }

    /// Sütun adını ekler; yinelenen adlara `~2`, `~3` soneki konur.
    fn sutun_ad_ekle(&mut self, ad: String) {
        let mut son = ad.clone();
        let mut n = 1;
        while self.sutun_indeks.contains_key(&son) {
            n += 1;
            son = format!("{ad}~{n}");
        }
        self.sutun_indeks.insert(son.clone(), self.basliklar.len());
        self.basliklar.push(son);
    }

    /// CSV/TSV kaydı okur. Dosya sonuysa `false`, geçerli kayıt okunduysa `true`.
    fn csv_kaydi_oku(&mut self) -> Sonuc<bool> {
        if !self.ham_kayit_oku()? {
            return Ok(false);
        }
        self.tampon.clear();
        self.tampon.push_str(&String::from_utf8_lossy(&self.ham));
        let mut acilan = std::mem::take(&mut self.acilan);
        let alanlar = csv_alan_ayir(&self.tampon, self.ayrac, &mut acilan, &mut self.bozuk_satir);
        self.acilan = acilan;
        self.alanlar = alanlar;
        Ok(true)
    }

    /// Sonraki ham kaydı `self.ham` içine alır. Dosya sonuysa `false`.
    fn ham_kayit_oku(&mut self) -> Sonuc<bool> {
        self.ham.clear();
        loop {
            let ok = self
                .girdi
                .read_until(b'\n', &mut self.ham)
                .map_err(|kaynak| Hata::Io {
                    islem: "okuma",
                    yol: self.yol.clone(),
                    kaynak,
                })?;
            if ok == 0 {
                return Ok(!self.ham.is_empty());
            }
            if self.ham.len() > MAKS_KAYIT_BAYTI {
                self.bozuk_satir += 1;
                self.satir_no += 1;
                self.ham_doldur();
                return Ok(true);
            }
            if !tirnak_dengesi(&self.ham) {
                return Ok(true);
            }
        }
    }

    /// Aşırı uzun kaydın kalan baytlarını bir sonraki satır sonuna kadar atar.
    fn ham_doldur(&mut self) {
        let mut atlandi = false;
        loop {
            self.ham.clear();
            match self.girdi.read_until(b'\n', &mut self.ham) {
                Ok(0) => return,
                Ok(_) => {
                    if !atlandi && tirnak_dengesi(&self.ham) {
                        atlandi = true;
                        continue;
                    }
                    return;
                }
                Err(_) => return,
            }
        }
    }

    /// JSONL kaydı okur. Akış bittiyse `false`.
    ///
    /// Boş değerler ve ayrıştırılamayan JSON satırları **atlanır**; bozuk satır
    /// sayacı artar ve akış bir sonraki değere devam eder (rapor b07: tek bozuk
    /// satır grafiği öldürmemeli).
    fn json_kaydi_oku(&mut self) -> Sonuc<bool> {
        let mut ham = String::with_capacity(256);
        loop {
            let aldi = {
                let akis = &mut self.json_akisi;
                let girdi = &mut *self.girdi;
                akis.sonraki(girdi, &mut ham)
            };
            if !aldi {
                self.json_bitti = true;
                return Ok(false);
            }
            if ham.trim().is_empty() {
                continue;
            }
            let deger: serde_json::Value = match serde_json::from_str(&ham) {
                Ok(d) => d,
                Err(_) => {
                    // Bozuk JSON: sayaç artar, `self.alanlar` **bozulmaz**,
                    // çünkü bozuk kayıt hiç üretilmez.
                    self.bozuk_satir += 1;
                    self.satir_no += 1;
                    continue;
                }
            };
            self.satir_no += 1;
            self.json_alana_cevir(&deger);
            return Ok(true);
        }
    }

    /// JSON değerini `self.alanlar` + `self.acilan` çiftine yazar.
    fn json_alana_cevir(&mut self, deger: &serde_json::Value) {
        self.tampon.clear();
        self.acilan.clear();
        self.alanlar.clear();
        match deger {
            serde_json::Value::Object(harita) => {
                let anahtarlar: Vec<String> = harita.keys().cloned().collect();
                for ad in anahtarlar.iter() {
                    if self.sutun_indeksi(ad).is_none() {
                        self.sutun_ad_ekle(ad.clone());
                    }
                }
                for ad in anahtarlar.iter() {
                    let i = self.sutun_indeksi(ad).unwrap_or(0);
                    if let Some(v) = harita.get(ad) {
                        self.alanlara_yaz(i, v);
                    }
                }
            }
            diger => {
                if self.basliklar.is_empty() {
                    self.sutun_ad_ekle("deger".to_string());
                }
                self.alanlara_yaz(0, diger);
            }
        }
    }

    /// Tek bir JSON değerini verilen sütun indeksine yazar.
    fn alanlara_yaz(&mut self, indeks: usize, deger: &serde_json::Value) {
        while self.alanlar.len() <= indeks {
            self.alanlar.push(Alan::Bos);
        }
        let alan = match deger {
            serde_json::Value::Null => Alan::Bos,
            serde_json::Value::Bool(b) => {
                let s = if *b { "true" } else { "false" };
                let bas = self.acilan.len();
                self.acilan.push_str(s);
                Alan::Acik {
                    bas: bas as u32,
                    bit: self.acilan.len() as u32,
                }
            }
            serde_json::Value::String(s) => {
                if s.is_empty() {
                    Alan::Bos
                } else if let Some(v) = sayi_ayiristir(s) {
                    Alan::Sayi(v)
                } else {
                    let bas = self.acilan.len();
                    self.acilan.push_str(s);
                    Alan::Acik {
                        bas: bas as u32,
                        bit: self.acilan.len() as u32,
                    }
                }
            }
            serde_json::Value::Number(n) => match n.as_f64() {
                Some(v) => Alan::Sayi(v),
                None => Alan::Bos,
            },
            // İç içe değerler düz metin olarak gösterilir; grafik için anlamsızdır.
            diger => {
                let s = diger.to_string();
                let bas = self.acilan.len();
                self.acilan.push_str(&s);
                Alan::Acik {
                    bas: bas as u32,
                    bit: self.acilan.len() as u32,
                }
            }
        };
        self.alanlar[indeks] = alan;
    }
}

/// Kayıt metnini alanlara böler ve her alanı sınıflandırır.
fn csv_alan_ayir(tampon: &str, ayrac: u8, acilan: &mut String, bozuk: &mut u64) -> Vec<Alan> {
    let bayt = tampon.as_bytes();
    let n = bayt.len();
    let mut i = 0usize;
    let mut alanlar: Vec<Alan> = Vec::with_capacity(8);
    loop {
        if i < n && bayt[i] == b'"' {
            let (alan, sonraki) = tirnakli_alan(tampon, i, ayrac, acilan, bozuk);
            alanlar.push(alan);
            i = sonraki;
        } else {
            let bas = i;
            while i < n && bayt[i] != ayrac && bayt[i] != b'\n' && bayt[i] != b'\r' {
                i += 1;
            }
            let mut bit = i;
            while bit > bas && (bayt[bit - 1] == b' ' || bayt[bit - 1] == b'\t') {
                bit -= 1;
            }
            alanlar.push(siniflandir_duz(tampon, bas, bit));
        }
        if i >= n {
            break;
        }
        match bayt[i] {
            b'\n' | b'\r' => {
                // Satır sonu: kaydın kalanını yutmaya gerek yok, döngü biter.
                break;
            }
            _ => {
                i += 1;
            }
        }
    }
    alanlar
}

/// `tampon` içinde `bas` konumundaki tırnaklı alanı çözer; sonraki konumu döndürür.
fn tirnakli_alan(
    tampon: &str,
    bas: usize,
    ayrac: u8,
    acilan: &mut String,
    bozuk: &mut u64,
) -> (Alan, usize) {
    let bayt = tampon.as_bytes();
    let n = bayt.len();
    let acilan_bas = acilan.len();
    let mut i = bas + 1;
    let mut kacis = false;
    let mut kapali = false;
    while i < n {
        let c = bayt[i];
        if kacis {
            acilan.push(c as char);
            kacis = false;
            i += 1;
            continue;
        }
        match c {
            b'"' => {
                if bayt.get(i + 1) == Some(&b'"') {
                    acilan.push('"');
                    i += 2;
                } else {
                    kapali = true;
                    i += 1;
                    break;
                }
            }
            b'\\' => {
                if bayt.get(i + 1) == Some(&b'"') {
                    acilan.push('"');
                    i += 2;
                } else {
                    kacis = true;
                    i += 1;
                }
            }
            _ => {
                let ch = tampon[i..].chars().next().unwrap_or('\u{fffd}');
                acilan.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    if !kapali {
        *bozuk += 1;
    }
    // Tırnak sonrası ek karakterler (gevşek CSV) içeriğe eklenir.
    while i < n && bayt[i] != ayrac && bayt[i] != b'\n' && bayt[i] != b'\r' {
        let ch = tampon[i..].chars().next().unwrap_or('\u{fffd}');
        acilan.push(ch);
        i += ch.len_utf8();
    }
    let bit = acilan.len();
    (siniflandir_acilan(acilan, acilan_bas, bit), i)
}

fn siniflandir_duz(tampon: &str, bas: usize, bit: usize) -> Alan {
    let s = &tampon[bas..bit];
    if s.is_empty() {
        return Alan::Bos;
    }
    if let Some(v) = sayi_ayiristir(s) {
        return Alan::Sayi(v);
    }
    Alan::Duz {
        bas: bas as u32,
        bit: bit as u32,
    }
}

fn siniflandir_acilan(acilan: &str, bas: usize, bit: usize) -> Alan {
    let s = &acilan[bas.min(acilan.len())..bit.min(acilan.len())];
    if s.is_empty() {
        return Alan::Bos;
    }
    if let Some(v) = sayi_ayiristir(s) {
        return Alan::Sayi(v);
    }
    Alan::Acik {
        bas: bas as u32,
        bit: bit as u32,
    }
}

/// Metni sayıya çevirir; tanınmazsa `None`.
///
/// Kabul: `42`, `-3.5`, `+1e9`, `.5`, `inf`, `-inf`, `NaN`, çevre boşluklu varyantları.
/// Reddedilen: boş/yalnız boşluk, `1,5` (ondalık ayracı değil), `1_000` (binlik ayraç).
pub fn sayi_ayiristir(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Tırnak dengesi: `"` sayısı tek ise kayıt satır sonunda bitmemiştir.
fn tirnak_dengesi(bayt: &[u8]) -> bool {
    bayt.iter().filter(|b| **b == b'"').count() % 2 == 1
}

/// Dosyanın ilk `ONEK_BAYTI` baytını okur (dosya kısaysa tamamı).
fn oku_onek(dosya: &File) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; ONEK_BAYTI];
    let mut al = dosya;
    let mut toplam = 0usize;
    loop {
        match al.read(&mut buf[toplam..]) {
            Ok(0) => break,
            Ok(n) => {
                toplam += n;
                if toplam == ONEK_BAYTI {
                    break;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    buf.truncate(toplam);
    Ok(buf)
}

/// Uzantı ve içerikten biçim + ayraç belirler.
fn bicim_ve_ayrac(yol: &Path, onek: &[u8], secenek: &AcSecenekleri) -> Sonuc<(Bicim, u8)> {
    let uzanti = yol
        .extension()
        .and_then(|u| u.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let json_mi = matches!(uzanti.as_str(), "jsonl" | "ndjson" | "json");
    if json_mi || (uzanti.is_empty() && json_kok_dizi_mi(onek)) {
        let dizi = json_kok_dizi_mi(onek);
        return Ok((
            if dizi {
                Bicim::JsonlDizi
            } else {
                Bicim::JsonlSatir
            },
            0,
        ));
    }
    if let Some(a) = secenek.ayrac {
        if a == b'"' || a == b'\n' || a == b'\r' {
            return Err(Hata::GecersizAyrac {
                deger: (a as char).to_string(),
            });
        }
        return Ok((if a == b'\t' { Bicim::Tsv } else { Bicim::Csv }, a));
    }
    match uzanti.as_str() {
        "tsv" => return Ok((Bicim::Tsv, b'\t')),
        "csv" => return Ok((Bicim::Csv, b',')),
        _ => {}
    }
    if json_kok_dizi_mi(onek) || onek.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        return Ok((Bicim::JsonlSatir, 0));
    }
    match en_sik_ayrac(onek) {
        Some(a) => Ok((if a == b'\t' { Bicim::Tsv } else { Bicim::Csv }, a)),
        None => Err(Hata::BilinmeyenBicim {
            yol: yol.to_path_buf(),
        }),
    }
}

/// Önek boşluk atlandıktan sonra `[` ile başlıyorsa kök dizi vardır.
fn json_kok_dizi_mi(onek: &[u8]) -> bool {
    onek.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'[')
}

/// Tırnak dışındaki en sık ayraç karakterini seçer; bağ yoksa `None`.
fn en_sik_ayrac(onek: &[u8]) -> Option<u8> {
    let adaylar = *b",\t;|";
    let mut sayilar = [0usize; 4];
    let mut tirnakli = false;
    for &b in onek {
        if b == b'"' {
            tirnakli = !tirnakli;
            continue;
        }
        if b == b'\n' {
            tirnakli = false;
            continue;
        }
        if tirnakli {
            continue;
        }
        for (i, a) in adaylar.iter().enumerate() {
            if b == *a {
                sayilar[i] += 1;
            }
        }
    }
    let mut en_iyi: Option<(usize, u8)> = None;
    for (i, a) in adaylar.iter().enumerate() {
        if sayilar[i] == 0 {
            continue;
        }
        match en_iyi {
            Some((m, _)) if m >= sayilar[i] => {}
            _ => en_iyi = Some((sayilar[i], *a)),
        }
    }
    en_iyi.map(|(_, a)| a)
}

/// JSON kök akış tarayıcısı: NDJSON satır değerleri veya kök dizi öğeleri.
///
/// **Tek bayt** okur; böylece ne dizi ne de satır **tümü** belleğe alınmaz.
/// Çıktı bayt tamponuna biriktirilir ve yalnızca tamamlandığında UTF-8'e
/// çevrilir; geçersiz bayt dizisi kayıt sınırında bozuk sayılır.
#[derive(Debug)]
pub struct JsonAkisi {
    dizi_modu: bool,
    dizi_acildi: bool,
    kapandi: bool,
    derinlik: i32,
    tirnakli: bool,
    kacis: bool,
    deger_bas: bool,
}

impl JsonAkisi {
    /// Yeni akış; `dizi_modu` kökün `[]` olduğunu belirtir.
    pub fn yeni(dizi_modu: bool) -> Self {
        Self {
            dizi_modu,
            dizi_acildi: !dizi_modu,
            kapandi: false,
            derinlik: 0,
            tirnakli: false,
            kacis: false,
            deger_bas: false,
        }
    }

    /// Bir sonraki tam JSON değerini `cikti` tamponuna yazar.
    ///
    /// Dönüş: `true` değer üretildi, `false` akış tükendi. Hatalı JSON metni de
    /// `true` döner; ayrıştırma `Okuyucu` içinde yapılır ve bozuk kayıt sayacını
    /// besler. Böylece bozuk satır grafiği öldürmez.
    pub fn sonraki(&mut self, girdi: &mut dyn BufRead, cikti: &mut String) -> bool {
        cikti.clear();
        if self.kapandi {
            return false;
        }
        let mut ham: Vec<u8> = Vec::with_capacity(256);
        loop {
            let c = match bayt_al(girdi) {
                Some(c) => c,
                None => {
                    self.kapandi = true;
                    if self.deger_bas {
                        cikti.push_str(&String::from_utf8_lossy(&ham));
                        self.sifirla();
                        return true;
                    }
                    return false;
                }
            };
            if self.tirnakli {
                if self.kacis {
                    self.kacis = false;
                    ham.push(c);
                } else {
                    match c {
                        b'\\' => self.kacis = true,
                        b'"' => {
                            self.tirnakli = false;
                            ham.push(c);
                            if self.derinlik == 0 {
                                cikti.push_str(&String::from_utf8_lossy(&ham));
                                self.sifirla();
                                return true;
                            }
                        }
                        _ => ham.push(c),
                    }
                }
                continue;
            }
            match c {
                b' ' | b'\t' | b'\r' => {}
                b'\n' => {
                    if self.deger_bas && self.derinlik == 0 {
                        cikti.push_str(&String::from_utf8_lossy(&ham));
                        self.sifirla();
                        return true;
                    }
                }
                b'[' if !self.dizi_acildi && !self.deger_bas => {
                    self.dizi_modu = true;
                    self.dizi_acildi = true;
                }
                b'[' | b'{' => {
                    self.deger_bas = true;
                    self.derinlik += 1;
                    ham.push(c);
                }
                b']' if self.derinlik == 0 => {
                    // Dizi kapandı: varsa son skaler değer önce teslim edilir.
                    self.kapandi = true;
                    if self.deger_bas {
                        cikti.push_str(&String::from_utf8_lossy(&ham));
                        self.sifirla();
                        return true;
                    }
                    return false;
                }
                b']' | b'}' => {
                    if !self.deger_bas {
                        self.kapandi = true;
                        return false;
                    }
                    ham.push(c);
                    self.derinlik -= 1;
                    if self.derinlik == 0 {
                        if c == b']' && self.dizi_modu {
                            self.kapandi = true;
                        }
                        cikti.push_str(&String::from_utf8_lossy(&ham));
                        self.sifirla();
                        return true;
                    }
                }
                b',' if self.derinlik == 0 => {
                    if self.deger_bas {
                        cikti.push_str(&String::from_utf8_lossy(&ham));
                        self.sifirla();
                        return true;
                    }
                }
                b'"' => {
                    self.deger_bas = true;
                    self.tirnakli = true;
                    ham.push(c);
                }
                _ => {
                    self.deger_bas = true;
                    ham.push(c);
                }
            }
            if ham.len() > MAKS_KAYIT_BAYTI {
                // Aşırı büyük değer: kayıt bozuk sayılır, akış devam eder.
                self.sifirla();
                cikti.clear();
                continue;
            }
        }
    }

    /// Bir değer teslim edildikten sonra durumu sıfırlar.
    fn sifirla(&mut self) {
        self.derinlik = 0;
        self.tirnakli = false;
        self.kacis = false;
        self.deger_bas = false;
    }
}

/// Girdiden tek bayt okur; `Interrupted` hatalarını yeniden dener.
fn bayt_al(girdi: &mut dyn Read) -> Option<u8> {
    let mut b = [0u8; 1];
    loop {
        match girdi.read(&mut b) {
            Ok(0) => return None,
            Ok(_) => return Some(b[0]),
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_yardimcisi::GeciciDizin;

    /// Test verisini geçici bir dosyaya yazar ve okuyucu açar.
    fn ac(etiket: &str, ad: &str, icerik: &str) -> (Okuyucu, GeciciDizin) {
        ac_ayar(etiket, ad, icerik, AcSecenekleri::default())
    }

    fn ac_ayar(etiket: &str, ad: &str, icerik: &str, ac: AcSecenekleri) -> (Okuyucu, GeciciDizin) {
        let g = GeciciDizin::yeni(etiket);
        let yol = g.yaz(ad, icerik).unwrap_or_else(|_| panic!("yazilamadi"));
        match Okuyucu::ac(&yol, &ac) {
            Ok(v) => (v, g),
            Err(e) => panic!("acma hatasi: {e}"),
        }
    }

    /// Okuyucudan tüm geçerli kayıtları toplar.
    fn tumunu_oku(o: &mut Okuyucu) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        loop {
            match o.sonraki_satir() {
                Ok(None) => break,
                Ok(Some(s)) => {
                    let mut r = Vec::new();
                    for i in 0..s.alan_sayisi() {
                        r.push(s.metin(i));
                    }
                    out.push(r);
                }
                Err(e) => panic!("okuma hatasi: {e}"),
            }
        }
        out
    }

    #[test]
    fn basit_csv_ayristirilir() {
        let (mut o, _g) = ac("a-temel", "v.csv", "a,b\n1,2\n3,4\n");
        assert_eq!(o.basliklar(), ["a", "b"]);
        let s = tumunu_oku(&mut o);
        assert_eq!(s, vec![vec!["1", "2"], vec!["3", "4"]]);
    }

    #[test]
    fn tirnakli_alan_ayrac_iceriyor() {
        let (mut o, _g) = ac("a-ic-ayrac", "v.csv", "a,b\n\"x,y\",2\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s[0][0], "x,y", "alan ici ayrac korunmali: {s:?}");
    }

    #[test]
    fn tirnakli_alan_satir_sonu_iceriyor() {
        let (mut o, _g) = ac("a-ic-satir", "v.csv", "a,b\n\"x\ny\",2\n3,4\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s.len(), 2, "kayitlar birlestirilmeli: {s:?}");
        assert_eq!(s[0][0], "x\ny");
    }

    #[test]
    fn kacisli_tirnak_cozulur() {
        let (mut o, _g) = ac("a-kacis", "v.csv", "a\n\"he said \"\"hi\"\"\"\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(
            s[0][0], "he said \"hi\"",
            "cift tirnak tek tirnaga donmeli: {s:?}"
        );
    }

    #[test]
    fn gevsek_kacisli_tirnak_cozulur() {
        let (mut o, _g) = ac("a-gevsek", "v.csv", "a\n\"he said \\\"hi\\\"\"\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s[0][0], "he said \"hi\"", "ters tirnak kacisi: {s:?}");
    }

    #[test]
    fn bos_alan_bos_kayit_uretir() {
        let (mut o, _g) = ac("a-bos", "v.csv", "a,b,c\n1,,3\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s[0][1], "", "orta alan bos olmali");
        assert_eq!(s[0][2], "3");
    }

    #[test]
    fn satir_sonu_ayrac_ek_bos_alan_uretir() {
        // RFC 4180 uyumlu davranış: satır sonundaki ayraç **bir** boş alan daha
        // üretir. Başlık iki sütun olduğu için bu kayıt bozuk sayılır ve atlanır.
        let (mut o, _g) = ac("a-son", "v.csv", "a,b\n1,2\n4,5,\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s, vec![vec!["1", "2"]], "ek alanli kayit atlanmali: {s:?}");
        assert_eq!(o.bozuk_satir_sayisi(), 1);
    }

    #[test]
    fn satir_sonu_ayraci_baslikla_uyumluysa_kabul_edilir() {
        let (mut o, _g) = ac("a-son2", "v.csv", "a,b,c\n1,2,\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s, vec![vec!["1", "2", ""]], "bos son alan korunmali: {s:?}");
    }

    #[test]
    fn bozuk_satir_atlanir_ve_sayilir() {
        let (mut o, _g) = ac("a-bozuk", "v.csv", "a,b\n1,2\n3\n4,5\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(
            s,
            vec![vec!["1", "2"], vec!["4", "5"]],
            "bozuk satir atlanmali"
        );
        assert_eq!(o.bozuk_satir_sayisi(), 1, "bozuk sayaci artmali");
    }

    #[test]
    fn tsv_uzantisiyla_sekme_ayraci_secilir() {
        let (mut o, _g) = ac("a-tsv", "v.tsv", "a\tb\n1\t2\n");
        assert_eq!(o.bicim(), Bicim::Tsv);
        let s = tumunu_oku(&mut o);
        assert_eq!(s[0], vec!["1", "2"]);
    }

    #[test]
    fn elle_ayrac_uygulanir() {
        let (mut o, _g) = ac_ayar(
            "a-ayrac",
            "v.dat",
            "a;b\n1;2\n",
            AcSecenekleri {
                ayrac: Some(b';'),
                baslik: true,
            },
        );
        assert_eq!(o.ayrac(), b';');
        let s = tumunu_oku(&mut o);
        assert_eq!(s[0], vec!["1", "2"]);
    }

    #[test]
    fn gecersiz_ayrac_reddedilir() {
        let g = GeciciDizin::yeni("a-gecersiz");
        let yol = g
            .yaz("v.csv", "a\n1\n")
            .unwrap_or_else(|_| panic!("yazilamadi"));
        let r = Okuyucu::ac(
            &yol,
            &AcSecenekleri {
                ayrac: Some(b'"'),
                baslik: true,
            },
        );
        assert!(r.is_err(), "tirnak ayrac olarak verilemez");
    }

    #[test]
    fn basliksiz_dosyada_sutunlar_uretilir() {
        let (mut o, _g) = ac_ayar(
            "a-basliksiz",
            "v.csv",
            "1,2\n3,4\n",
            AcSecenekleri {
                ayrac: None,
                baslik: false,
            },
        );
        assert_eq!(o.basliklar(), ["s1", "s2"]);
        let s = tumunu_oku(&mut o);
        assert_eq!(s.len(), 2, "ilk satir da veri sayilmali");
    }

    #[test]
    fn yinelenen_basliklar_ayirt_edilir() {
        let (mut o, _g) = ac("a-yinelenen", "v.csv", "a,a,a\n1,2,3\n");
        assert_eq!(o.basliklar(), ["a", "a~2", "a~3"]);
        let s = tumunu_oku(&mut o);
        assert_eq!(s[0], vec!["1", "2", "3"], "veri yer degistirmemeli");
    }

    #[test]
    fn sayisal_alanlar_sayi_olarak_donusur() {
        let (mut o, _g) = ac("a-sayi", "v.csv", "a,b\n1,2.5\n");
        match o.sonraki_satir() {
            Ok(Some(s)) => {
                assert_eq!(s.sayi(0), Some(1.0));
                assert_eq!(s.sayi(1), Some(2.5));
            }
            _ => panic!("kayit okunamadi"),
        }
    }

    #[test]
    fn nan_ve_inf_ayristirilir() {
        let (mut o, _g) = ac("a-nan", "v.csv", "a\nNaN\ninf\n-inf\n");
        match o.sonraki_satir() {
            Ok(Some(s)) => assert!(s.sayi(0).map(f64::is_nan).unwrap_or(false)),
            _ => panic!("kayit okunamadi"),
        }
        match o.sonraki_satir() {
            Ok(Some(s)) => assert_eq!(s.sayi(0), Some(f64::INFINITY)),
            _ => panic!("kayit okunamadi"),
        }
    }

    #[test]
    fn jsonl_satir_koku_okunur() {
        let (mut o, _g) = ac(
            "a-jsonl-satir",
            "v.jsonl",
            "{\"t\":1,\"v\":10}\n{\"t\":2,\"v\":20}\n",
        );
        assert_eq!(o.bicim(), Bicim::JsonlSatir);
        let s = tumunu_oku(&mut o);
        assert_eq!(s.len(), 2, "iki kayit okunmali: {s:?}");
        assert_eq!(
            o.basliklar(),
            ["t", "v"],
            "anahtarlar alfabetik: {:?}",
            o.basliklar()
        );
    }

    #[test]
    fn jsonl_dizi_koku_okunur() {
        let (mut o, _g) = ac(
            "a-jsonl-dizi",
            "v.jsonl",
            "[\n  {\"a\": 1},\n  {\"a\": 2},\n  {\"a\": 3}\n]\n",
        );
        assert_eq!(o.bicim(), Bicim::JsonlDizi);
        let s = tumunu_oku(&mut o);
        assert_eq!(s.len(), 3, "dizi ogeleri ayri kayit olmali: {s:?}");
        assert_eq!(s[2][0], "3");
    }

    #[test]
    fn jsonl_tek_satirlik_dizi_okunur() {
        let (mut o, _g) = ac("a-jsonl-tek", "v.jsonl", "[{\"a\":1},{\"a\":2}]");
        let s = tumunu_oku(&mut o);
        assert_eq!(s.len(), 2, "{s:?}");
    }

    #[test]
    fn jsonl_bozuk_satir_atlanir() {
        let (mut o, _g) = ac("a-jsonl-bozuk", "v.jsonl", "{\"a\":1}\nbozuk\n{\"a\":2}\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(s.len(), 2, "gecerli kayitlar okunmali: {s:?}");
        assert!(o.bozuk_satir_sayisi() >= 1, "bozuk sayaci artmali");
    }

    #[test]
    fn jsonl_yeni_anahtarlar_sutun_ekler() {
        let (mut o, _g) = ac("a-jsonl-yeni", "v.jsonl", "{\"a\":1}\n{\"a\":2,\"b\":9}\n");
        let s = tumunu_oku(&mut o);
        assert_eq!(o.basliklar(), ["a", "b"], "yeni anahtar sona eklenmeli");
        assert_eq!(s[1][1], "9");
    }

    #[test]
    fn jsonl_sayisal_degerler_sayi_olur() {
        let (mut o, _g) = ac("a-jsonl-sayi", "v.jsonl", "{\"a\":3.5,\"b\":\"7\"}\n");
        match o.sonraki_satir() {
            Ok(Some(s)) => {
                assert_eq!(s.sayi(0), Some(3.5));
                assert_eq!(s.sayi(1), Some(7.0), "sayisal metin sayiya cevrilmeli");
            }
            _ => panic!("kayit okunamadi"),
        }
    }

    #[test]
    fn bilinmeyen_bicim_reddedilir() {
        let g = GeciciDizin::yeni("a-bilinmez");
        // Ne ayraç içeren ne de JSON ile başlayan içerik.
        let yol = g
            .yaz("v.bin", "\u{0}\u{1}\u{2}")
            .unwrap_or_else(|_| panic!("yazilamadi"));
        assert!(Okuyucu::ac(&yol, &AcSecenekleri::default()).is_err());
    }

    #[test]
    fn dosya_acma_hatasi_donusur() {
        let g = GeciciDizin::yeni("a-yok");
        let yol = g.yol().join("yok.csv");
        assert!(Okuyucu::ac(&yol, &AcSecenekleri::default()).is_err());
    }

    #[test]
    fn sayi_ayiristir_kurallari() {
        assert_eq!(sayi_ayiristir("42"), Some(42.0));
        assert_eq!(sayi_ayiristir(" -3.5 "), Some(-3.5));
        assert_eq!(sayi_ayiristir("+1e3"), Some(1000.0));
        assert_eq!(sayi_ayiristir(".5"), Some(0.5));
        assert_eq!(sayi_ayiristir(""), None);
        assert_eq!(sayi_ayiristir("   "), None);
        assert_eq!(sayi_ayiristir("1,5"), None, "ondalik ayrac degildir");
        assert_eq!(sayi_ayiristir("1_000"), None, "binlik ayrac desteklenmez");
        assert_eq!(sayi_ayiristir("abc"), None);
    }

    #[test]
    fn tirnak_dengesi_tespit_edilir() {
        assert!(tirnak_dengesi(b"a,\"b"));
        assert!(!tirnak_dengesi(b"a,\"b\",c"));
    }

    #[test]
    fn en_sik_ayrac_guvenilir() {
        assert_eq!(en_sik_ayrac(b"a,b,c\n1,2,3\n"), Some(b','));
        assert_eq!(en_sik_ayrac(b"a\tb\tc\n1\t2\n"), Some(b'\t'));
        assert_eq!(en_sik_ayrac(b"tek sutun\nveri\n"), None);
    }
}
