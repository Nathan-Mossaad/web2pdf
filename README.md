web2pdf
=====================
This project consists of two parts:
1. A CLI tool that converts web pages to PDFs
2. A wrapper around [chromiumoxide](https://github.com/mattsse/chromiumoxide) that allows for a more streamlined experience when creating PDFs

The main new feature compared to other PDF converters is the ability to create a single page PDF, that fits to the content, instead of a standard multi-page PDF.

It also allows provides the ability to use screen instead of printing CSS, thereby converting exactly what the user sees on the screen.

# Setup
1. Install chromium
2. ```cargo install web2pdf```

## Usage
For the CLI tool, run "web2pdf --help"

Examle usage:

```web2pdf --mono --screen --disable-backgrounds "https://en.wikipedia.org/wiki/Rust_(programming_language)" rust.pdf```

```web2pdf "document.html" rust.pdf```\
(Warning: Paths relative to home are not supported e.g. ```~/document.html``` but ```/home/user/document.html``` will work)

## License

Licensed under either of these:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

