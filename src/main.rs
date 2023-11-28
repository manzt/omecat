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

fn get_ome_ifd_index(sel: Selection, pixels: &Pixels) -> usize {
    // TODO: handle multiple diff data
    let image_offset = 0;
    let Pixels {
        size_t,
        size_c,
        size_z,
        dimension_order,
        ..
    } = pixels;
    let Selection { t, z, c } = sel;
    match dimension_order.as_str() {
        "XYZCT" => image_offset + t * size_z * size_c + c * size_z + z,
        "XYZTC" => image_offset + c * size_z * size_t + t * size_z + z,
        "XYCTZ" => image_offset + z * size_c * size_t + t * size_c + c,
        "XYCZT" => image_offset + t * size_c * size_z + z * size_c + c,
        "XYTCZ" => image_offset + z * size_t * size_c + c * size_t + t,
        "XYTZC" => image_offset + c * size_t * size_z + z * size_t + t,
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
    fn filename(&self, z: usize) -> String {
        match self.size_z {
            1..=9 => self.filename_template.replace("{z}", &format!("{:01}", z)),
            10..=99 => self.filename_template.replace("{z}", &format!("{:02}", z)),
            100..=999 => self.filename_template.replace("{z}", &format!("{:03}", z)),
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
            let ifd = get_ome_ifd_index(Selection { t: 0, z, c }, &image.pixels);
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
