use std::sync::Arc;

use hayro::hayro_interpret::{
    font::{FontData, FontQuery, StandardFont},
    InterpreterSettings,
};
use hayro::hayro_syntax::Pdf;
use hayro_svg::{RenderCache, SvgRenderSettings};

fn standard_font(font: StandardFont) -> Option<(FontData, u32)> {
    let bytes: &'static [u8] = match font {
        StandardFont::Helvetica => typst_assets::pdf::SANS,
        StandardFont::HelveticaBold => typst_assets::pdf::SANS_BOLD,
        StandardFont::HelveticaOblique => typst_assets::pdf::SANS_ITALIC,
        StandardFont::HelveticaBoldOblique => typst_assets::pdf::SANS_BOLD_ITALIC,
        StandardFont::Courier => typst_assets::pdf::FIXED,
        StandardFont::CourierBold => typst_assets::pdf::FIXED_BOLD,
        StandardFont::CourierOblique => typst_assets::pdf::FIXED_ITALIC,
        StandardFont::CourierBoldOblique => typst_assets::pdf::FIXED_BOLD_ITALIC,
        StandardFont::TimesRoman => typst_assets::pdf::SERIF,
        StandardFont::TimesBold => typst_assets::pdf::SERIF_BOLD,
        StandardFont::TimesItalic => typst_assets::pdf::SERIF_ITALIC,
        StandardFont::TimesBoldItalic => typst_assets::pdf::SERIF_BOLD_ITALIC,
        StandardFont::ZapfDingBats => typst_assets::pdf::DING_BATS,
        StandardFont::Symbol => typst_assets::pdf::SYMBOL,
    };
    Some((Arc::new(bytes), 0))
}

pub fn first_page_to_svg(pdf: Vec<u8>) -> Result<String, String> {
    let pdf = Pdf::new(pdf).map_err(|e| format!("cannot read the PDF: {e:?}"))?;
    let page = pdf.pages().first().ok_or("the PDF has no pages")?;
    let settings = InterpreterSettings {
        font_resolver: Arc::new(|query| match query {
            FontQuery::Standard(font) => standard_font(*font),
            FontQuery::Fallback(fallback) => standard_font(fallback.pick_standard_font()),
        }),
        cmap_resolver: Arc::new(|_| None),
        warning_sink: Arc::new(|_| {}),
        render_annotations: false,
    };
    let cache = RenderCache::new();
    Ok(hayro_svg::convert(page, &cache, &settings, &SvgRenderSettings { bg_color: [0, 0, 0, 0] }))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn tiny_pdf() -> Vec<u8> {
        let content = "0.8 0.2 0.2 rg 5 5 40 20 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 50 30] /Contents 4 0 R /Resources << >> >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        ];
        let mut out = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
        }
        let xref = out.len();
        out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
        for offset in offsets {
            out.push_str(&format!("{offset:010} 00000 n \n"));
        }
        out.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1));
        out.into_bytes()
    }

    #[test]
    fn a_pdf_page_becomes_an_svg_with_its_drawing() {
        let svg = first_page_to_svg(tiny_pdf()).unwrap();
        assert!(svg.starts_with("<svg"), "{svg}");
        assert!(svg.contains("<path"), "{svg}");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(first_page_to_svg(b"not a pdf".to_vec()).is_err());
    }
}
