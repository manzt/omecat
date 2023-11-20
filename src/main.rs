use serde::{Deserialize, Serialize};
use quick_xml::de::from_str;
use quick_xml::se::to_string;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OME {
    #[serde(rename = "Image", default)]
    images: Vec<Image>
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
    #[serde(rename="Channel", default)]
    channels: Vec<Channel>,
    #[serde(rename="TiffData", default)]
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
    #[serde(rename="LightPath")]
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
    #[serde(rename="UUID")]
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
    let Pixels { size_t, size_c, size_z, dimension_order, .. } = pixels;
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
}

fn to_multifile_companion_ome(xml_str: &str, config: &StackConfig) -> anyhow::Result<OME> {
    let mut src: OME = from_str(xml_str)?;
    let image = src.images.first_mut().unwrap();
    // Clear out the existing TiffData
    image.pixels.physical_size_z = Some(config.physical_size_z);
    image.pixels.physical_size_z_unit = Some(config.physical_size_z_unit.clone());
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
                    file_name: format!("{}.ome.tif", z),
                }),
            };
            image.pixels.tiff_data.push(tiff_data);
        }
    }
    Ok(src)
}

fn main() -> anyhow::Result<()> {
    if let Some(fp) = std::env::args().nth(1) {
        let reader = std::fs::File::open(fp).map(std::io::BufReader::new)?;
        let mut decoder = tiff::decoder::Decoder::new(reader)?;

        if let Some(tiff::decoder::ifd::Value::Ascii(s)) =
            decoder.find_tag(tiff::tags::Tag::ImageDescription)?
        {
            let doc: xmlem::Document = s.parse()?;
            let s = doc.to_string_pretty();
            println!("{}", &s);
            let ome = to_multifile_companion_ome(&s, &StackConfig {
                size_z: 10,
                physical_size_z: 1.0,
                physical_size_z_unit: "µm".to_string(),
            })?;
            let s = to_string(&ome)?;
            let doc: xmlem::Document = s.parse()?;
            let s = doc.to_string_pretty();
            println!("{}", &s);
        }
    } else {
        println!("Usage: omecat <filename>");
    }
    Ok(())
}
