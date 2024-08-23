//vendored from `image` @ 0.25.1

use std::ffi::OsStr;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
#[allow(dead_code)]
pub enum ImageFormat {
    /// An Image in PNG Format
    Png,

    /// An Image in JPEG Format
    Jpeg,

    /// An Image in GIF Format
    Gif,

    /// An Image in WEBP Format
    WebP,

    /// An Image in general PNM Format
    Pnm,

    /// An Image in TIFF Format
    Tiff,

    /// An Image in TGA Format
    Tga,

    /// An Image in DDS Format
    Dds,

    /// An Image in BMP Format
    Bmp,

    /// An Image in ICO Format
    Ico,

    /// An Image in Radiance HDR Format
    Hdr,

    /// An Image in OpenEXR Format
    OpenExr,

    /// An Image in farbfeld Format
    Farbfeld,

    /// An Image in AVIF format.
    Avif,

    /// An Image in QOI format.
    Qoi,
}

static MAGIC_BYTES: [(&[u8], ImageFormat); 23] = [
    (b"\x89PNG\r\n\x1a\n", ImageFormat::Png),
    (&[0xff, 0xd8, 0xff], ImageFormat::Jpeg),
    (b"GIF89a", ImageFormat::Gif),
    (b"GIF87a", ImageFormat::Gif),
    (b"RIFF", ImageFormat::WebP), // TODO: better magic byte detection, see https://github.com/image-rs/image/issues/660
    (b"MM\x00*", ImageFormat::Tiff),
    (b"II*\x00", ImageFormat::Tiff),
    (b"DDS ", ImageFormat::Dds),
    (b"BM", ImageFormat::Bmp),
    (&[0, 0, 1, 0], ImageFormat::Ico),
    (b"#?RADIANCE", ImageFormat::Hdr),
    (b"P1", ImageFormat::Pnm),
    (b"P2", ImageFormat::Pnm),
    (b"P3", ImageFormat::Pnm),
    (b"P4", ImageFormat::Pnm),
    (b"P5", ImageFormat::Pnm),
    (b"P6", ImageFormat::Pnm),
    (b"P7", ImageFormat::Pnm),
    (b"farbfeld", ImageFormat::Farbfeld),
    (b"\0\0\0 ftypavif", ImageFormat::Avif),
    (b"\0\0\0\x1cftypavif", ImageFormat::Avif),
    (&[0x76, 0x2f, 0x31, 0x01], ImageFormat::OpenExr), // = &exr::meta::magic_number::BYTES
    (b"qoif", ImageFormat::Qoi),
];

impl ImageFormat {
    pub fn guess_format(buffer: &[u8]) -> Option<Self> {
        for &(signature, format) in &MAGIC_BYTES {
            if buffer.starts_with(signature) {
                return Some(format);
            }
        }
        None
    }

    pub fn extensions_str(self) -> &'static [&'static str] {
        match self {
            ImageFormat::Png => &["png"],
            ImageFormat::Jpeg => &["jpg", "jpeg"],
            ImageFormat::Gif => &["gif"],
            ImageFormat::WebP => &["webp"],
            ImageFormat::Pnm => &["pbm", "pam", "ppm", "pgm"],
            ImageFormat::Tiff => &["tiff", "tif"],
            ImageFormat::Tga => &["tga"],
            ImageFormat::Dds => &["dds"],
            ImageFormat::Bmp => &["bmp"],
            ImageFormat::Ico => &["ico"],
            ImageFormat::Hdr => &["hdr"],
            ImageFormat::OpenExr => &["exr"],
            ImageFormat::Farbfeld => &["ff"],
            // According to: https://aomediacodec.github.io/av1-avif/#mime-registration
            ImageFormat::Avif => &["avif"],
            ImageFormat::Qoi => &["qoi"],
        }
    }

    pub fn to_mime_type(self) -> &'static str {
        match self {
            ImageFormat::Avif => "image/avif",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Tiff => "image/tiff",
            // the targa MIME type has two options, but this one seems to be used more
            ImageFormat::Tga => "image/x-targa",
            ImageFormat::Dds => "image/vnd-ms.dds",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Hdr => "image/vnd.radiance",
            ImageFormat::OpenExr => "image/x-exr",
            // return the most general MIME type
            ImageFormat::Pnm => "image/x-portable-anymap",
            // Qoi's MIME type is being worked on.
            // See: https://github.com/phoboslab/qoi/issues/167
            ImageFormat::Qoi => "image/x-qoi",
            // farbfeld's MIME type taken from https://www.wikidata.org/wiki/Q28206109
            ImageFormat::Farbfeld => "application/octet-stream",
        }
    }

    #[inline]
    pub fn from_extension<S>(ext: S) -> Option<Self>
    where
        S: AsRef<OsStr>,
    {
        // thin wrapper function to strip generics
        fn inner(ext: &OsStr) -> Option<ImageFormat> {
            let ext = ext.to_str()?.to_ascii_lowercase();

            Some(match ext.as_str() {
                "avif" => ImageFormat::Avif,
                "jpg" | "jpeg" => ImageFormat::Jpeg,
                "png" => ImageFormat::Png,
                "gif" => ImageFormat::Gif,
                "webp" => ImageFormat::WebP,
                "tif" | "tiff" => ImageFormat::Tiff,
                "tga" => ImageFormat::Tga,
                "dds" => ImageFormat::Dds,
                "bmp" => ImageFormat::Bmp,
                "ico" => ImageFormat::Ico,
                "hdr" => ImageFormat::Hdr,
                "exr" => ImageFormat::OpenExr,
                "pbm" | "pam" | "ppm" | "pgm" => ImageFormat::Pnm,
                "ff" => ImageFormat::Farbfeld,
                "qoi" => ImageFormat::Qoi,
                _ => return None,
            })
        }

        inner(ext.as_ref())
    }
}
