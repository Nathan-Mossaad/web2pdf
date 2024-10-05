use std::future::Future;
use std::path::Path;
use tokio::fs;

use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use chromiumoxide::cdp::browser_protocol::target::CreateTargetParams;
use chromiumoxide::handler::viewport::Viewport;
use chromiumoxide::page::MediaTypeParams;
use chromiumoxide::Page;
use futures::StreamExt;

pub use chromiumoxide::browser::Browser;
pub use chromiumoxide::browser::BrowserConfig;
pub mod util;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub trait BrowserWeb2Pdf {
    fn web2pdf_launch_from_config(
        browser_config: BrowserConfig,
    ) -> impl Future<Output = Result<Browser>> + Send;
    fn web2pdf_launch() -> impl Future<Output = Result<Browser>> + Send;
    fn web2pdf_launch_from_executable_path(
        path: impl AsRef<Path> + Send,
    ) -> impl Future<Output = Result<Browser>> + Send;
    fn close_and_wait(self) -> impl Future<Output = Result<Browser>> + Send;
    fn web2pdf_new_page(
        &self,
        params: impl Into<CreateTargetParams> + Send,
    ) -> impl Future<Output = Result<Page>> + Send;
    fn web2pdf_load_cookie_file(
        &self,
        file: impl AsRef<Path> + Send,
    ) -> impl Future<Output = Result<()>> + Send;
}

pub trait PageWeb2Pdf {
    fn web2pdf_save_pdf_standard(
        &self,
        output: impl AsRef<Path> + Send,
    ) -> impl Future<Output = chromiumoxide::Result<Vec<u8>>> + Send;
    fn web2pdf_save_pdf_mono(
        &self,
        opts: PrintToPdfParams,
        output: impl AsRef<Path> + Send,
    ) -> impl Future<Output = chromiumoxide::Result<Vec<u8>>> + Send;
    fn web2pdf_save_pdf_mono_standard(
        &self,
        output: impl AsRef<Path> + Send,
    ) -> impl Future<Output = chromiumoxide::Result<Vec<u8>>> + Send;
}

pub trait ViewportWeb2Pdf {
    fn web2pdf_viewport() -> Viewport;
}

impl BrowserWeb2Pdf for Browser {
    /// Creates a new `Browser` instance from a given `BrowserConfig`.
    /// Please remeber to use Viewport::web2pdf_viewport() for building the config
    ///
    /// # Arguments
    /// * `browser_config` - The `BrowserConfig` to use for launching the browser.
    ///
    /// # Returns
    /// A `Result` containing a new `Web2Pdf` instance or an error.
    fn web2pdf_launch_from_config(
        browser_config: BrowserConfig,
    ) -> impl Future<Output = Result<Browser>> + Send {
        async {
            let (browser, mut handler) = Browser::launch(browser_config).await?;

            // Spawn a task to handle the browser events
            let _ = tokio::spawn(async move { while let Some(_) = handler.next().await {} });

            tracing::debug!("Web2Pdf browser launched");

            Ok(browser)
        }
    }

    /// Creates a new `Browser` instance using the system's installed Chromium browser.
    ///
    /// # Returns
    /// A `Result` containing a new `Browser` instance or an error.
    fn web2pdf_launch() -> impl Future<Output = Result<Browser>> + Send {
        async {
            // Attempt to find a system installation of chromium
            let browser_config = BrowserConfig::builder()
                .viewport(Some(Viewport::web2pdf_viewport()))
                .build()?;

            tracing::debug!("Web2Pdf browser launching using standard config");

            Self::web2pdf_launch_from_config(browser_config).await
        }
    }

    /// Creates a new `Browser` instance using a specific Chromium executable path.
    ///
    /// # Arguments
    /// * `path` - A path to the Chromium executable.
    ///
    /// # Returns
    /// A `Result` containing a new `Browser` instance or an error.
    fn web2pdf_launch_from_executable_path(
        path: impl AsRef<Path> + Send,
    ) -> impl Future<Output = Result<Browser>> + Send {
        async move {
            // Load a browser from a specific executable path
            let browser_config = BrowserConfig::builder()
                .viewport(Some(Viewport::web2pdf_viewport()))
                .chrome_executable(&path)
                .build()?;

            tracing::debug!("Web2Pdf browser launching using executable path");

            Self::web2pdf_launch_from_config(browser_config).await
        }
    }

    /// Closes the browser instance and waits for it to terminate.
    ///
    /// # Returns
    /// A `Result` containing an empty `()` value or an error.
    fn close_and_wait(mut self) -> impl Future<Output = Result<Browser>> + Send {
        async move {
            self.close().await?;
            self.wait().await?;
            Ok(self)
        }
    }

    /// Create a new browser page
    ///
    /// # Arguments
    /// * `params` - The `CreateTargetParams` to use for creating the page.
    ///
    /// # Returns
    /// A `Result` containing a new `Page` instance or an error.
    fn web2pdf_new_page(
        &self,
        params: impl Into<CreateTargetParams> + Send,
    ) -> impl Future<Output = Result<Page>> + Send {
        async move {
            let page = self.new_page(params).await?;
            page.emulate_media_type(MediaTypeParams::Print).await?;
            tracing::debug!("Web2Pdf new page created");
            Ok(page)
        }
    }

    /// Load a cookie file
    fn web2pdf_load_cookie_file(
        &self,
        file: impl AsRef<Path> + Send,
    ) -> impl Future<Output = Result<()>> + Send {
        async move {
            let file_contents = fs::read_to_string(file).await?;

            let cookies = util::parse_cookie_file(&file_contents)?;

            self.set_cookies(cookies).await?;
            Ok(())
        }
    }
}

impl PageWeb2Pdf for Page {
    /// Saves the page as a PDF file.
    ///
    /// # Arguments
    /// * `output` - The path to save the PDF file to.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<u8>` containing the PDF data or an error.
    /// (The Page is already saved as PDF at the specified path)
    fn web2pdf_save_pdf_standard(
        &self,
        output: impl AsRef<Path> + Send,
    ) -> impl Future<Output = chromiumoxide::Result<Vec<u8>>> + Send {
        async move {
            let pdf_params = PrintToPdfParams::builder()
                .print_background(true)
                .prefer_css_page_size(true)
                .build();
            let pdf = self.save_pdf(pdf_params, output).await?;

            Ok(pdf)
        }
    }

    /// Saves the page as a single PDF page
    ///
    /// # Note use web2pdf_launch or web2pdf_launch_from_executable_path for correct results
    /// # Arguments
    /// * `opts` - The `PrintToPdfParams` to use for saving the PDF.
    /// * `output` - The path to save the PDF file to.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<u8>` containing the PDF data or an error.
    /// (The Page is already saved as PDF at the specified path)
    fn web2pdf_save_pdf_mono(
        &self,
        mut opts: PrintToPdfParams,
        output: impl AsRef<Path> + Send,
    ) -> impl Future<Output = chromiumoxide::Result<Vec<u8>>> + Send {
        async move {
            let layout = self.layout_metrics().await?;

            opts.scale = None;
            opts.landscape = Some(false);

            // See: https://developer.mozilla.org/en-US/docs/Web/CSS/length#absolute_length_units
            opts.paper_height = Some(
                (layout.css_content_size.height / 96.0)
                    + opts.margin_top.unwrap_or(0.4)
                    + opts.margin_bottom.unwrap_or(0.4),
            );
            opts.paper_width = Some(
                (layout.css_content_size.width / 96.0)
                    + opts.margin_left.unwrap_or(0.4)
                    + opts.margin_right.unwrap_or(0.4),
            );

            // Some websites force a second (empty) page due to their CSS
            opts.page_ranges = Some("1".to_string());

            tracing::trace!("Web2Pdf mono page layout: {:?}", layout);

            let pdf = self.save_pdf(opts, output).await?;

            Ok(pdf)
        }
    }

    /// Saves the page as a single PDF page
    ///
    /// # Note use web2pdf_launch or web2pdf_launch_from_executable_path for correct results
    /// # Arguments
    /// * `output` - The path to save the PDF file to.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<u8>` containing the PDF data or an error.
    /// (The Page is already saved as PDF at the specified path)
    fn web2pdf_save_pdf_mono_standard(
        &self,
        output: impl AsRef<Path> + Send,
    ) -> impl Future<Output = chromiumoxide::Result<Vec<u8>>> + Send {
        async move {
            let opts = PrintToPdfParams::builder()
                .print_background(true)
                .prefer_css_page_size(true)
                .build();

            self.web2pdf_save_pdf_mono(opts, output).await
        }
    }
}

impl ViewportWeb2Pdf for Viewport {
    // Use standard a4 paper size as page size minus default border (8.268-2*0.4 x 11.693-2*0.4 (inches) * 96 (dpi))
    // See: https://developer.mozilla.org/en-US/docs/Web/CSS/length#absolute_length_units
    fn web2pdf_viewport() -> Viewport {
        Viewport {
            width: 717,
            height: 1046,
            device_scale_factor: Some(1.0),
            emulating_mobile: false,
            is_landscape: false,
            has_touch: false,
        }
    }
}
