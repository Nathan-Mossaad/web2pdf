use clap::Parser;
use futures::future::join_all;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

// Animations and logging
use tracing::{debug, error, info, instrument, trace};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use chromiumoxide::{cdp::browser_protocol::page::PrintToPdfParams, handler::viewport::Viewport};
use web2pdf_lib::{Browser, BrowserConfig, BrowserWeb2Pdf, PageWeb2Pdf, ViewportWeb2Pdf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub struct URLPathPair {
    pub url: String,
    pub path: PathBuf,
}

// A simple way to create PDFs from web pages
#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about = "A simple CLI tool to convert web pages to PDFs",
    long_about = "A simple CLI tool to convert web pages to PDFs\nReturns a non zero exit code equals to the amount of PDFs that couldn't be generated."
)]
pub struct Cli {
    #[clap(
        short = 'M',
        long = "mono",
        help = "Create a single page PDF, that fits to the content, instead of a standard multi-page PDF",
        long_help = "Create a single page PDF, that fits to the content, instead of a standard multi-page PDF\nThis will override other options like paper size, margins, etc.\nAdding a header or footer may cut of the content and is not advised.",
        default_value_t = false
    )]
    pub mono_page: bool,

    #[clap(
        short = 'S',
        long = "screen",
        help = "Emulates a screen media type (use standard CSS instead of printing CSS)",
        default_value_t = false
    )]
    pub screen_media_type: bool,

    // PDF Params taken from chromiumoxide_cdp
    #[clap(
        long,
        help = "Paper orientation",
        long_help = "Paper orientation. Sets paper orientation to landscape",
        default_value_t = false
    )]
    pub landscape: bool,
    #[clap(
        long = "disable-backgrounds",
        help = "Disable printing of background graphics",
        default_value_t = false
    )]
    pub disable_print_background: bool,
    #[clap(long, help = "Paper width in inches. Defaults to 8.5 inches")]
    pub paper_width: Option<f64>,
    #[clap(
        long,
        help = "Paper height in inches. Defaults to 11 inches",
        long_help = "Paper height in inches. Defaults to 11 inches.\nDue to a minimum printing width values below 6.5 inches result in unexpected behaviour."
    )]
    pub paper_height: Option<f64>,
    #[clap(
        long,
        help = "Top margin in inches. Defaults to 1cm (0.3937 inches)",
        default_value_t = 0.3937
    )]
    pub margin_top: f64,
    #[clap(
        long,
        help = "Bottom margin in inches. Defaults to 1cm (0.3937 inches)",
        default_value_t = 0.3937
    )]
    pub margin_bottom: f64,
    #[clap(
        long,
        help = "Left margin in inches. Defaults to 1cm (0.3937 inches)",
        default_value_t = 0.3937
    )]
    pub margin_left: f64,
    #[clap(
        long,
        help = "Right margin in inches. Defaults to 1cm (0.3937 inches)",
        default_value_t = 0.3937
    )]
    pub margin_right: f64,
    #[clap(
        long,
        help = "Page ranges to print, e.g., '1-5, 8, 11-13'",
        long_help = "Paper ranges to print, one based, e.g., '1-5, 8, 11-13'. Pages are\nprinted in the document order, not in the order specified, and no\nmore than once.\nDefaults to empty string, which implies the entire document is printed.\nThe page numbers are quietly capped to actual page count of the\ndocument, and ranges beyond the end of the document are ignored.\nIf this results in no pages to print, an error is reported.\nIt is an error to specify a range with start greater than end."
    )]
    pub page_ranges: Option<String>,
    #[clap(long, help = "Display header and footer", default_value_t = false)]
    pub display_header_footer: bool,
    #[clap(
        long,
        help = "HTML template for the print header",
        long_help = "HTML template for the print header. Should be valid HTML markup with following\nclasses used to inject printing values into them:\n- `date`: formatted print date\n- `title`: document title\n- `url`: document location\n- `pageNumber`: current page number\n- `totalPages`: total pages in the document\n\nFor example, `<span class=title></span>` would generate span containing the title."
    )]
    pub header_template: Option<String>,
    #[clap(
        long,
        help = "HTML template for the print footer.",
        long_help = "HTML template for the print footer. Should use the same format as the `headerTemplate`."
    )]
    pub footer_template: Option<String>,
    #[clap(
        long,
        help = "Disable prefering page size as defined by css",
        long_help = "Disable prefering page size as defined by css. Defaults to false,\nin which case the content will be scaled to fit the paper size.",
        default_value_t = false
    )]
    pub disable_prefer_css_page_size: bool,
    #[clap(
        long,
        help = "Whether or not to generate tagged (accessible) PDF. Defaults to embedder choice."
    )]
    pub generate_tagged_pdf: Option<bool>,
    // End of PDF Params
    #[clap(
        long,
        help = "Scale of the webpage rendering. Range from 0.1 to 2",
        long_help = "Scale of the webpage rendering. Range from 0.1 to 2\nWhen using --mono, this is ignored, use --paper-width instead."
    )]
    pub scale: Option<f64>,

    #[clap(
        long,
        help = "Path to a cookie jar file (in Netscape format), to be loaded into the browser"
    )]
    pub cookie_jar: Option<PathBuf>,

    #[clap(long, help = "Path to a (chromium) browser executable")]
    pub browser_path: Option<PathBuf>,

    #[clap(long, help = "Force ANSI output")]
    pub ansi_only: bool,

    #[clap(required = true, num_args = 2.., value_names = &["URL", "PATH"], help = "URL-Path pairs to convert to PDFs")]
    pub raw_url_path_pairs: Option<Vec<String>>,

    #[clap(skip)]
    pub url_path_pairs: Vec<URLPathPair>,
}

impl Cli {
    /// Constructs url_path_pairs from raw_url_path_pairs (Clears raw_url_path_pairs)
    ///
    /// # Panics
    /// Panics if raw_url_path_pairs is None
    /// Panics if the number of arguments is not even
    pub fn replace_url_path_pairs(mut self) -> Self {
        let raw_url_path_pairs = match self.raw_url_path_pairs {
            Some(raw_url_path_pairs) => raw_url_path_pairs,
            None => panic!("No URL-Path pairs provided: This function is only to be called once at the start of the program"),
        };

        // Check if url and path are multiple of 2
        if raw_url_path_pairs.len() % 2 != 0 {
            if self.ansi_only {
                eprintln!("error: URL-Path pairs must be in pairs of two, could not find a path for: \n{}\n", raw_url_path_pairs.last().unwrap());
                eprintln!("For more information, try '--help'.");
            } else {
                eprintln!("\x1b[31merror:\x1b[0m URL-Path pairs must be in pairs of two, could not find a path for: \n{}\n", raw_url_path_pairs.last().unwrap());
                eprintln!("For more information, try '\x1b[1m--help\x1b[0m'.");
            }
            std::process::exit(1);
        }

        let mut pairs: Vec<URLPathPair> = Vec::new();
        for pair in raw_url_path_pairs.chunks_exact(2) {
            pairs.push(URLPathPair {
                url: String::from(&pair[0]),
                path: PathBuf::from(&pair[1]),
            });
        }

        self.raw_url_path_pairs = None;
        self.url_path_pairs.append(&mut pairs);
        self
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let exit_code = Arc::new(Mutex::new(0));

    let mut cli = Cli::parse().replace_url_path_pairs();
    // Check if the first path refers to a file
    for pair in cli.url_path_pairs.iter_mut() {
        let path = Path::new(&pair.url);
        if path.is_file() {
            trace!("Path {} is a file, converting to file:// URL", path.display());
            pair.url = format!("file://{}", path.display());
        }
    }

    // Parse Cli args
    let cli = Arc::new(cli);

    // Start logging
    let indicatif_layer = IndicatifLayer::new();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
    );
    if cli.ansi_only {
        subscriber
            .with(tracing_subscriber::fmt::layer().with_ansi(false).compact())
            .init();
    } else {
        subscriber
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(indicatif_layer.get_stderr_writer())
                    .compact(),
            )
            .with(indicatif_layer)
            .init();
    }

    debug!("{:?}", cli);

    let browser = Arc::new({
        // Create viewport for browser config
        let mut viewport = Viewport::web2pdf_viewport();
        if let Some(scale) = &cli.scale {
            viewport.device_scale_factor = Some(*scale);
        }
        if let Some(width) = &cli.paper_width {
            viewport.width = (*width * 96.0) as u32;
        }
        if let Some(height) = &cli.paper_height {
            viewport.height = (*height * 96.0) as u32;
        }
        // Create browser config
        let mut browser_config = BrowserConfig::builder().viewport(Some(viewport));
        if let Some(path) = &cli.browser_path {
            browser_config = browser_config.chrome_executable(path);
        }
        let browser_config = browser_config.build()?;
        debug!("browser_config: {:?}", browser_config);

        // Attempt to start browser
        match Browser::web2pdf_launch_from_config(browser_config).await {
            Ok(browser) => browser,
            Err(e) => {
                error!("Failed to launch browser with reason: {}", e);
                std::process::exit(1);
            }
        }
    });

    browser.clear_cookies().await?;
    // Load cookies
    match &cli.cookie_jar {
        Some(cookie_file) => {
            debug!("Loading cookies from {:?}", cookie_file);
            match browser.web2pdf_load_cookie_file(cookie_file).await {
                Ok(_) => {}
                Err(e) => {
                    error!(
                        "Failed to load cookies from {:?} with reason: {}",
                        cookie_file, e
                    );
                    std::process::exit(1);
                }
            }
        }
        None => {}
    }

    // Create threads for each created pdf
    let tasks = (0..cli.url_path_pairs.len()).into_iter().map(|page_num| {
        let cli = Arc::clone(&cli);
        let browser = Arc::clone(&browser);
        let exit_code = Arc::clone(&exit_code);
        tokio::spawn(async move {
            let mut error = false;
            match pdf_tab(&cli, &browser, page_num).await {
                Ok(()) => {
                    info!("Created pdf from {}", cli.url_path_pairs[page_num].url);
                }
                Err(e) => {
                    error!(
                        "Error creating pdf from \"{}\" with reason: {}",
                        cli.url_path_pairs[page_num].url, e
                    );
                    error = true;
                }
            }
            if error {
                *exit_code.lock().await += 1;
            }
        })
    });

    join_all(tasks).await;

    // Close the browser
    Arc::try_unwrap(browser)
        .expect("Ganing ownership to close browser failed!")
        .close_and_wait()
        .await?;
    debug!("Closed browser");

    std::process::exit(*exit_code.lock().await);
}

/// Creates a PDF from cli and browser for a given page_num
///
/// # Arguments
/// * `cli` - The cli
/// * `browser` - The browser
/// * `page_num` - The nth element to create the PDF for
///
///
/// # Errors
/// Errors if the page could not be created
#[instrument(skip_all, name = "Creating PDF for ", fields(page = cli.url_path_pairs[page_num].url))]
async fn pdf_tab(cli: &Arc<Cli>, browser: &Arc<Browser>, page_num: usize) -> Result<()> {
    // PDF Params
    let mut pdf_params_builder = PrintToPdfParams::builder()
        .landscape(cli.landscape)
        .display_header_footer(cli.display_header_footer)
        .print_background(!cli.disable_print_background)
        .margin_top(cli.margin_top)
        .margin_bottom(cli.margin_bottom)
        .margin_left(cli.margin_left)
        .margin_right(cli.margin_right)
        .prefer_css_page_size(!cli.disable_prefer_css_page_size);

    if let Some(width) = &cli.paper_width {
        pdf_params_builder = pdf_params_builder.paper_width(*width);
    }
    if let Some(height) = &cli.paper_height {
        pdf_params_builder = pdf_params_builder.paper_height(*height);
    }
    if let Some(page_ranges) = &cli.page_ranges {
        pdf_params_builder = pdf_params_builder.page_ranges(page_ranges);
    }
    if let Some(header_template) = &cli.header_template {
        pdf_params_builder = pdf_params_builder.header_template(header_template);
    }
    if let Some(footer_template) = &cli.footer_template {
        pdf_params_builder = pdf_params_builder.footer_template(footer_template);
    }
    if let Some(scale) = &cli.scale {
        pdf_params_builder = pdf_params_builder.scale(*scale);
    }
    let pdf_params = pdf_params_builder.build();

    let pair = &cli.url_path_pairs[page_num];

    let page = browser.web2pdf_new_page(&pair.url).await?;

    if cli.screen_media_type {
        page.emulate_media_type(chromiumoxide::page::MediaTypeParams::Screen)
            .await?;
    }

    if cli.mono_page {
        page.web2pdf_save_pdf_mono(pdf_params, &pair.path).await?;
    } else {
        page.save_pdf(pdf_params, &pair.path).await?;
    }

    page.close().await?;

    Ok(())
}
