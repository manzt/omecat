use clap::{Parser, Subcommand};
use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OME {
    #[serde(rename = "Image", default)]
    images: Vec<Image>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Image {
    #[serde(rename = "@ID")]
    id: String,
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "Pixels")]
    pixels: Pixels,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Pixels {
    #[serde(rename = "@ID")]
    id: String,
    #[serde(rename = "@Type")]
    r#type: String,
    #[serde(rename = "@SizeX")]
    size_x: usize,
    #[serde(rename = "@SizeY")]
    size_y: usize,
    #[serde(rename = "@SizeZ")]
    size_z: usize,
    #[serde(rename = "@SizeC")]
    size_c: usize,
    #[serde(rename = "@SizeT")]
    size_t: usize,
    #[serde(rename = "@PhysicalSizeX")]
    physical_size_x: Option<f64>,
    #[serde(rename = "@PhysicalSizeXUnit")]
    physical_size_x_unit: Option<String>,
    #[serde(rename = "@PhysicalSizeY")]
    physical_size_y: Option<f64>,
    #[serde(rename = "@PhysicalSizeYUnit")]
    physical_size_y_unit: Option<String>,
    #[serde(rename = "@PhysicalSizeZ")]
    physical_size_z: Option<f64>,
    #[serde(rename = "@PhysicalSizeZUnit")]
    physical_size_z_unit: Option<String>,
    #[serde(rename = "@DimensionOrder")]
    dimension_order: String,
    #[serde(rename = "Channel", default)]
    channels: Vec<Channel>,
    #[serde(rename = "TiffData", default)]
    tiff_data: Vec<TiffData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Channel {
    #[serde(rename = "@ID")]
    id: String,
    #[serde(rename = "@SamplesPerPixel")]
    samples_per_pixel: usize,
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "LightPath")]
    light_path: LightPath,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LightPath {}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TiffData {
    #[serde(rename = "@IFD")]
    ifd: Option<usize>,
    #[serde(rename = "@PlaneCount")]
    plane_count: Option<usize>,
    #[serde(rename = "@FirstC")]
    first_c: Option<usize>,
    #[serde(rename = "@FirstZ")]
    first_z: Option<usize>,
    #[serde(rename = "@FirstT")]
    first_t: Option<usize>,
    #[serde(rename = "UUID")]
    uuid: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Uuid {
    #[serde(rename(serialize = "@FileName", deserialize = "FileName"))]
    file_name: String,
}

struct Selection {
    t: usize,
    z: usize,
    c: usize,
}

fn get_relative_ifd_index(selection: Selection, pixels: &Pixels) -> usize {
    let Pixels { size_t, size_c, size_z, .. } = pixels;
    let Selection { t, z, c } = selection;
    match pixels.dimension_order.as_str() {
        "XYZCT" => z + (size_z * c) + (size_z * size_c * t),
        "XYZTC" => z + (size_z * t) + (size_z * size_t * c),
        "XYCTZ" => c + (size_c * t) + (size_c * size_t * z),
        "XYCZT" => c + (size_c * z) + (size_c * size_z * t),
        "XYTCZ" => t + (size_t * c) + (size_t * size_c * z),
        "XYTZC" => t + (size_t * z) + (size_t * size_z * c),
        _ => panic!("Invalid dimension order"),
    }
}

struct StackConfig {
    size_z: usize,
    physical_size_z: f64,
    physical_size_z_unit: String,
    filename_template: String,
}

impl StackConfig {
    /// Returns the filename for the given z index
    /// The z index is 0-based
    /// The filename is 1-based
    /// The filename is zero-padded to the number of digits in size_z
    /// e.g. size_z = 10, z = 0, filename = 01
    /// e.g. size_z = 100, z = 0, filename = 001
    /// e.g. size_z = 100, z = 99, filename = 100
    fn filename(&self, z: usize) -> String {
        match self.size_z {
            1..=9 => self.filename_template.replace("{z}", &format!("{:02}", z + 1)),
            10..=99 => self.filename_template.replace("{z}", &format!("{:02}", z + 1)),
            100..=999 => self.filename_template.replace("{z}", &format!("{:03}", z + 1)),
            _ => panic!("Invalid size_z"),
        }
    }
}

fn to_multifile_companion_ome(xml_str: &str, config: &StackConfig) -> anyhow::Result<OME> {
    let mut src: OME = from_str(xml_str)?;
    let image = src.images.first_mut().unwrap();

    image.pixels.physical_size_z = Some(config.physical_size_z);
    image.pixels.physical_size_z_unit = Some(config.physical_size_z_unit.clone());

    // Clear out the existing TiffData
    image.pixels.tiff_data.clear();
    assert_eq!(image.pixels.size_t, 1);

    for z in 0..config.size_z {
        for (c, _) in image.pixels.channels.iter().enumerate() {
            let ifd = get_relative_ifd_index(Selection { t: 0, z: 0, c }, &image.pixels);
            let tiff_data = TiffData {
                ifd: Some(ifd),
                plane_count: Some(1),
                first_c: Some(c),
                first_z: Some(z),
                first_t: Some(0),
                uuid: Some(Uuid {
                    file_name: config.filename(z),
                }),
            };
            image.pixels.tiff_data.push(tiff_data);
        }
    }

    image.pixels.size_z = config.size_z;
    Ok(src)
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(required = false)]
    file: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Adds files to myapp
    Concat {
        // Positional arguments specific to the Concat subcommand
        #[arg(required = true)]
        file: String,
        #[arg(long)]
        filename_template: String,
        #[arg(long)]
        size_z: usize,
        #[arg(long, default_value_t = 1.0)]
        physical_size_z: f64,
        #[arg(long, default_value = "µm")]
        physical_size_z_unit: String,
    },
}

fn get_image_description(file: &str) -> anyhow::Result<String> {
    let reader = std::fs::File::open(file).map(std::io::BufReader::new)?;
    let mut decoder = tiff::decoder::Decoder::new(reader)?;
    if let Some(tiff::decoder::ifd::Value::Ascii(s)) =
        decoder.find_tag(tiff::tags::Tag::ImageDescription)?
    {
        Ok(s)
    } else {
        Err(anyhow::anyhow!("No ImageDescription tag found"))
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    match &cli.command {
        Some(Commands::Concat {
            file,
            size_z,
            physical_size_z,
            physical_size_z_unit,
            filename_template,
        }) => {
            let xml_str = get_image_description(file)?;
            let ome = to_multifile_companion_ome(
                &xml_str,
                &StackConfig {
                    size_z: *size_z,
                    physical_size_z: *physical_size_z,
                    physical_size_z_unit: physical_size_z_unit.to_string(),
                    filename_template: filename_template.to_string(),
                },
            )?;
            let doc: xmlem::Document = to_string(&ome)?.parse()?;
            handle.write_all(doc.to_string_pretty().as_bytes())?;
        }
        None => {
            if let Some(file) = &cli.file {
                let xml_str = get_image_description(file)?;
                let doc: xmlem::Document = xml_str.parse()?;
                handle.write_all(doc.to_string_pretty().as_bytes())?;
            }
        }
    }
    Ok(())
}
