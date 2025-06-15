mod types;

use std::{
    fs::File,
    io::{self, BufWriter, Read, Write},
};

use types::{Channels, Colorspace};

fn main() {
    println!("Hello, world!");

    let mut data = Vec::new();
    let width = 32;
    let height = 32;
    for i in 0..height {
        for _ in 0..width {
            if i % 2 == 0 {
                data.push([255, 255, 255, 245]);
            } else {
                data.push([0, 0, 0, 255]);
            }
        }
    }
    create_file(width, height, Colorspace::Standard, &data).unwrap();

    let res = encode_rgba(&data);
    const TAG_MASK: u8 = 0b11_000000;
    let mut rgba = 0u8;
    let mut rgb = 0;
    let mut luma = false;
    for byte in res {
        let data = byte & !TAG_MASK;
        if luma {
            println!("Luma: {data} ");
            continue;
        }
        if rgba > 0 {
            match rgba {
                4 | 3 | 2 => {
                    print!(" {data}")
                }
                1 => println!(" {data}"),
                _ => unreachable!(),
            }
            rgba -= 1;
            continue;
        }
        if rgb > 0 {
            match rgb {
                3 | 2 => {
                    print!(" {data:b}")
                }
                1 => println!(" {data:b}"),
                _ => unreachable!(),
            }
            rgb -= 1;
            continue;
        }
        match byte {
            RGB_TAG => {
                print!("RGB");
                rgb = 3;
            }
            RGBA_TAG => {
                println!("RGBA");
                rgba = 4;
            }
            _ => match byte & TAG_MASK {
                INDEX_TAG => println!("Index: {data}"),
                DIFF_TAG => println!(
                    "Diff: {} {} {}",
                    (data >> 4) as i8 - 2,
                    ((data >> 2) & 0b11) as i8 - 2,
                    (data & 0b11) as i8 - 2
                ),
                LUMA_TAG => {
                    print!("Luma: {data:b}");
                    luma = true;
                }
                RUN_TAG => println!("Run of: {}", data + 1),
                _ => panic!(),
            },
        }
    }
}

const INDEX_TAG: u8 = 0b00 << 6;
const DIFF_TAG: u8 = 0b01 << 6;
const LUMA_TAG: u8 = 0b10 << 6;
const RUN_TAG: u8 = 0b11 << 6;
const RGB_TAG: u8 = 0b11111110;
const RGBA_TAG: u8 = 0b11111111;

const MAGIC: &[u8; 4] = b"qoif";
const END_STREAM: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

fn create_file(
    width: u32,
    height: u32,
    colorspace: Colorspace,
    pixels: &[[u8; 4]],
) -> io::Result<()> {
    let encoded = encode_rgba(pixels);
    let mut file = BufWriter::new(File::create("test.qoi")?);
    // Header
    file.write_all(MAGIC)?;
    file.write_all(&width.to_be_bytes())?;
    file.write_all(&height.to_be_bytes())?;
    file.write_all(&[Channels::Rgba as u8])?;
    file.write_all(&[colorspace as u8])?;
    // Data
    file.write_all(&encoded)?;
    // End
    file.write_all(&END_STREAM)?;

    Ok(())
}

fn read_file() -> io::Result<Vec<[u8; 4]>> {
    let mut file = File::open("test.qoi")?;
    let mut buff4 = [0; 4];
    let mut buff1 = [0; 1];

    file.read(&mut buff4)?;
    assert_eq!(&buff4, b"qoif", "Header must start with 'qoif'");

    file.read(&mut buff4)?;
    let _width = u32::from_be_bytes(buff4);

    file.read(&mut buff4)?;
    let _height = u32::from_be_bytes(buff4);

    file.read(&mut buff1)?;
    let channels = Channels::try_from(buff1[0])
        .expect("Invalid number of color channels. Supported are 3 (rgb) or 4 (rgba)");

    file.read(&mut buff1)?;
    let _colorspace = Colorspace::try_from(buff1[0])
        .expect("Unsupported colorspace. Valid options are 0 (sRGB) or 1 (Linear RGB)");

    let mut encoded = Vec::new();
    file.read_to_end(&mut encoded)?;
    let bytes = match channels {
        Channels::Rgba => decode_rgba(&encoded),
        _ => unimplemented!(),
    };
    Ok(bytes)
}

fn encode_rgba(pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut encoded = Vec::new();
    let mut seen = [[0, 0, 0, 0]; 64];
    let mut prev = [0, 0, 0, 255];
    let mut i = 0;
    while i < pixels.len() {
        let pixel = pixels[i];
        let index = hash(pixel);

        if pixel == prev {
            let mut run = 0;
            while run < 61 && i + 1 < pixels.len() && pixels[i + 1] == prev {
                run += 1;
                i += 1;
            }
            encoded.push(RUN_TAG | run);
        } else if seen[index as usize] == pixel {
            encoded.push(INDEX_TAG | index);
        } else if let Some(data) = diff(prev, pixel) {
            encoded.push(DIFF_TAG | data);
        } else if let Some((green_diff, red_blue_diff)) = luma(prev, pixel) {
            encoded.push(LUMA_TAG | green_diff);
            encoded.push(red_blue_diff);
        } else {
            encoded.push(RGBA_TAG);
            encoded.extend(pixel);
        }
        prev = pixel;
        i += 1;
        seen[index as usize] = pixel;
    }
    encoded
}

fn decode_rgba(mut encoded: &[u8]) -> Vec<[u8; 4]> {
    const TAG_MASK: u8 = 0b11_000000;
    let mut previous = [0, 0, 0, 255];
    let mut seen = [[0; 4]; 64];
    let mut pixels = Vec::new();
    while let [byte, remainder @ ..] = encoded {
        let byte = *byte;
        encoded = remainder;
        let pixel = match byte {
            RGB_TAG => {
                let [r, g, b, remainder @ ..] = encoded else {
                    panic!("RGB data missing");
                };
                encoded = remainder;
                [*r, *g, *b, previous[3]]
            }
            RGBA_TAG => {
                let [r, g, b, a, remainder @ ..] = encoded else {
                    panic!("RGBA data missing");
                };
                encoded = remainder;
                [*r, *g, *b, *a]
            }
            _ => {
                let tag = byte & TAG_MASK;
                let data = byte & !TAG_MASK;
                match tag {
                    INDEX_TAG => seen[byte as usize], // Works because INDEX_TAG is just leading 0s
                    DIFF_TAG => {
                        let red_diff = data >> 4;
                        let green_diff = (data >> 2) & 0b11;
                        let blue_diff = data & 0b11;
                        [
                            previous[0] + red_diff - 2,
                            previous[1] + green_diff - 2,
                            previous[2] + blue_diff - 2,
                            previous[3], // Unchanged opacity
                        ]
                    }
                    LUMA_TAG => {
                        let [red_blue_data, remainder @ ..] = encoded else {
                            panic!("Luma data missing");
                        };
                        encoded = remainder;
                        let green_diff = data;
                        let red_diff = red_blue_data >> 4 + green_diff;
                        let blue_diff = red_blue_data & 0b1111 + green_diff;
                        [
                            previous[0] + red_diff - 8,
                            previous[1] + green_diff - 32,
                            previous[2] + blue_diff - 8,
                            previous[3],
                        ]
                    }
                    RUN_TAG => {
                        for _ in 0..data {
                            pixels.push(previous);
                        }
                        seen[hash(previous) as usize] = previous; // Necessary only if this is the first operation
                        previous // The push below corrects for the -1 bias
                    }
                    _ => panic!("Unexpected tag"),
                }
            }
        };
        pixels.push(pixel);
        previous = pixel;
    }
    pixels
}

type Pixel = [u8; 4];

fn hash(pixel: Pixel) -> u8 {
    let [r, g, b, a] = pixel;
    (r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) as u8 % 64
}

fn diff(x: Pixel, y: Pixel) -> Option<u8> {
    let [rx, gx, bx, ax] = x;
    let [ry, gy, by, ay] = y;
    if ax != ay {
        return None;
    }
    let red_data = ry.wrapping_sub(rx).wrapping_add(2);
    let green_data = gy.wrapping_sub(gx).wrapping_add(2);
    let blue_data = by.wrapping_sub(bx).wrapping_add(2);
    if red_data < 4 && green_data < 4 && blue_data < 4 {
        Some(red_data << 4 | green_data << 2 | blue_data << 0)
    } else {
        None
    }
}

fn luma(x: Pixel, y: Pixel) -> Option<(u8, u8)> {
    let [rx, gx, bx, ax] = x;
    let [ry, gy, by, ay] = y;
    if ax != ay {
        return None;
    }
    let red_diff = rx.wrapping_sub(ry);
    let green_diff = gx.wrapping_sub(gy);
    let blue_diff = bx.wrapping_sub(by);

    let red_data = (red_diff - green_diff).wrapping_add(8);
    let green_data = green_diff.wrapping_add(32);
    let blue_data = (blue_diff - green_diff).wrapping_add(8);

    if red_data < 64 && green_data < 16 && blue_data < 16 {
        Some((green_data, red_data << 4 | blue_data))
    } else {
        None
    }
}
