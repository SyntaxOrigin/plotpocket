//! EğriKutu (PlotPocket) çekirdek kütüphanesi.
//!
//! Amaç: büyük CSV/TSV/JSONL dosyalarından **kayan tamponla** okuma, **LTTB**
//! indirgeme, bir **çizim listesi** üretimi ve bu listeden hem terminal karakter
//! ızgarası hem de bağımsız SVG metni çıkarma.
//!
//! Katmanlar tek yönlü bağımlılık gösterir (rapor b06):
//!
//! ```text
//! hata  <-  ayristirici  <-  profil
//! hata  <-  bicim (ölçek) <-  lttb  <-  sablon  <-  cizim  <-  terminal / svg
//!                                                       ^
//!                                                       |
//!                                                 canli (yalnız dosya + çizim)
//! ```
//!
//! Kritik mimari karar (rapor b07): grafik bir belge nesnesi **değil**, ekran
//! koordinatlarının bir listesidir. Aynı `CizimListesi` terminal çıktısını da
//! SVG metnini de besler; iki ayrı grafik kütüphanesi gerekmez.
//!
//! # Güvenlik
//!
//! Tüm kaynak `#![forbid(unsafe_code)]` ile derlenir. Grafik arayüzü (WebView,
//! `egui`, `winit`), görsel kodlayıcı (`image`, `tiny-skia`) ve dosya izleme
//! (`notify`) kütüphaneleri **kullanılmaz** (bkz. `WORKER_CONTRACT.md` § 3.2-F, § 3.2-G).

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod ayristirici;
pub mod bicim;
pub mod canli;
pub mod cizim;
pub mod hata;
pub mod lttb;
pub mod profil;
pub mod sablon;
pub mod svg;
pub mod terminal;

#[cfg(test)]
mod test_yardimcisi;
