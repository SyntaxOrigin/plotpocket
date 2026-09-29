//! Yalnızca birim testlerde kullanılan geçici dosya yardımcısı.
//!
//! Neden `tempfile` yok: bağımlılık politikası (`WORKER_CONTRACT.md` § 3.2)
//! `tempfile`'i hiçbir projede vermez; yardımcı kendi kodumuzla yazılır.
//!
//! Benzersizlik `std::process::id()` + etiketten gelir; rastgelelik crate'i
//! kullanılmaz. `Drop` ile temizlik yapılır.

use std::io;
use std::path::{Path, PathBuf};

/// Test içinde geçici dosya/dizin üreten, `Drop` ile temizleyen kapsayıcı.
pub(crate) struct GeciciDizin {
    yol: PathBuf,
}

impl GeciciDizin {
    /// `std::env::temp_dir()` altında, etiketten türetilmiş bir dizin oluşturur.
    ///
    /// Geçici dizin oluşturulamazsa test zaten çalışamayacağı için `expect`
    /// ile durulur; gerekçe: bu yalnızca test kodudur ve `WORKER_CONTRACT.md`
    /// § 4.2 uyarınca `expect` testlerde gerekçeli kullanılabilir.
    pub(crate) fn yeni(etiket: &str) -> Self {
        let kok = std::env::temp_dir().join(format!("{etiket}-{}", std::process::id()));
        // Aynı etiketle ikinci bir çalıştırma eski içeriği bulabilir; temizle.
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(&kok)
            .unwrap_or_else(|e| panic!("gecici dizin olusturulamadi: {kok:?} ({e})"));
        Self { yol: kok }
    }

    /// Dizin içine göreli yol döndürür.
    pub(crate) fn yol(&self) -> &Path {
        &self.yol
    }

    /// Dizin içine göreli yola içerik yazar.
    pub(crate) fn yaz(&self, ad: &str, icerik: &str) -> io::Result<PathBuf> {
        use std::io::Write as _;
        let yol = self.yol.join(ad);
        let mut f = std::fs::File::create(&yol)?;
        f.write_all(icerik.as_bytes())?;
        Ok(yol)
    }
}

impl Drop for GeciciDizin {
    fn drop(&mut self) {
        // `Drop` içinden hata döndürülemez; temizlik başarısız olsa da testi
        // düşürmemelidir. Bu, "sessiz yutma" yasağının `Drop` temizliği
        // istisnasıdır (README'de belgelenmiştir).
        let _ = std::fs::remove_dir_all(&self.yol);
    }
}
