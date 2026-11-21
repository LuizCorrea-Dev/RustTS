use crate::bake_mesh::inflate_zlib;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DecodedPng {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct BakeTexturesReport {
    pub textures_baked: usize,
    pub pixels_total: usize,
    pub bytes_written: usize,
    pub names: Vec<String>,
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = (a as i32) + (b as i32) - (c as i32);
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

pub fn decode_png_rgba(raw: &[u8]) -> Result<DecodedPng, String> {
    const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if raw.len() < 33 || raw[..8] != SIG {
        return Err("Invalid PNG signature".to_string());
    }
    let mut w = 0u32;
    let mut h = 0u32;
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut idat = Vec::new();
    let mut pos = 8usize;
    while pos + 12 <= raw.len() {
        let clen = u32::from_be_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]]) as usize;
        let ctype = &raw[pos + 4..pos + 8];
        if pos + 12 + clen > raw.len() {
            break;
        }
        let cdata = &raw[pos + 8..pos + 8 + clen];
        if ctype == b"IHDR" && clen >= 13 {
            w = u32::from_be_bytes([cdata[0], cdata[1], cdata[2], cdata[3]]);
            h = u32::from_be_bytes([cdata[4], cdata[5], cdata[6], cdata[7]]);
            bit_depth = cdata[8];
            color_type = cdata[9];
        } else if ctype == b"IDAT" {
            idat.extend_from_slice(cdata);
        } else if ctype == b"IEND" {
            break;
        }
        pos += 12 + clen;
    }
    let bpp = match color_type {
        6 => 4usize,
        2 => 3usize,
        4 => 2usize,
        0 => 1usize,
        _ => return Err(format!("Unsupported PNG color_type {}", color_type)),
    };
    if w == 0 || h == 0 || bit_depth != 8 || idat.is_empty() {
        return Err("Unsupported or corrupt PNG IHDR/IDAT".to_string());
    }
    let raw_img = inflate_zlib(&idat)?;
    let stride = (w as usize) * bpp;
    let expected = (stride + 1) * (h as usize);
    if raw_img.len() < expected {
        return Err("Truncated decompressed PNG scanlines".to_string());
    }
    let mut unfilt = vec![0u8; (h as usize) * stride];
    for y in 0..h as usize {
        let ftype = raw_img[y * (stride + 1)];
        let src_row = &raw_img[y * (stride + 1) + 1..y * (stride + 1) + 1 + stride];
        let dst_start = y * stride;
        for i in 0..stride {
            let a = if i >= bpp { unfilt[dst_start + i - bpp] } else { 0 };
            let b = if y > 0 { unfilt[(y - 1) * stride + i] } else { 0 };
            let c = if y > 0 && i >= bpp {
                unfilt[(y - 1) * stride + i - bpp]
            } else {
                0
            };
            let v = match ftype {
                0 => src_row[i],
                1 => src_row[i].wrapping_add(a),
                2 => src_row[i].wrapping_add(b),
                3 => src_row[i].wrapping_add((((a as u16) + (b as u16)) >> 1) as u8),
                4 => src_row[i].wrapping_add(paeth_predictor(a, b, c)),
                _ => src_row[i],
            };
            unfilt[dst_start + i] = v;
        }
    }
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let si = y * stride + x * bpp;
            let di = (y * w as usize + x) * 4;
            match bpp {
                4 => rgba[di..di + 4].copy_from_slice(&unfilt[si..si + 4]),
                3 => {
                    rgba[di] = unfilt[si];
                    rgba[di + 1] = unfilt[si + 1];
                    rgba[di + 2] = unfilt[si + 2];
                    rgba[di + 3] = 255;
                }
                2 => {
                    rgba[di] = unfilt[si];
                    rgba[di + 1] = unfilt[si];
                    rgba[di + 2] = unfilt[si];
                    rgba[di + 3] = unfilt[si + 1];
                }
                _ => {
                    rgba[di] = unfilt[si];
                    rgba[di + 1] = unfilt[si];
                    rgba[di + 2] = unfilt[si];
                    rgba[di + 3] = 255;
                }
            }
        }
    }
    Ok(DecodedPng {
        width: w,
        height: h,
        rgba,
    })
}

fn collect_pngs_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed reading '{}': {}", dir.display(), e))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_pngs_recursive(&p, out)?;
        } else if p
            .extension()
            .and_then(|s| s.to_str())
            .map_or(false, |e| e.eq_ignore_ascii_case("png"))
        {
            out.push(p);
        }
    }
    Ok(())
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.as_bytes()[0].is_ascii_digit() {
        format!("TEX_{}", out)
    } else {
        out
    }
}

pub fn bake_textures_dir(input_dir: &Path, output_path: &Path) -> Result<BakeTexturesReport, String> {
    let mut files = Vec::new();
    collect_pngs_recursive(input_dir, &mut files)?;
    files.sort();

    let mut groups: BTreeMap<String, (Option<PathBuf>, Option<PathBuf>)> = BTreeMap::new();
    for f in files {
        let stem_owned = f
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let lower = stem_owned.to_ascii_lowercase();
        if let Some(base) = stem_owned.strip_suffix("_Color").or_else(|| {
            if lower.ends_with("_color") {
                Some(&stem_owned[..stem_owned.len() - 6])
            } else {
                None
            }
        }) {
            groups.entry(base.to_string()).or_default().0 = Some(f);
        } else if let Some(base) = stem_owned.strip_suffix("_AmbientOcclusion").or_else(|| {
            if lower.ends_with("_ambientocclusion") {
                Some(&stem_owned[..stem_owned.len() - 17])
            } else if lower.ends_with("_ao") {
                Some(&stem_owned[..stem_owned.len() - 3])
            } else {
                None
            }
        }) {
            groups.entry(base.to_string()).or_default().1 = Some(f);
        } else if !lower.ends_with("_normaldx")
            && !lower.ends_with("_metalness")
            && !lower.ends_with("_roughness")
        {
            groups.entry(stem_owned).or_default().0 = Some(f);
        }
    }

    let is_rts = output_path
        .extension()
        .and_then(|s| s.to_str())
        .map_or(false, |e| e.eq_ignore_ascii_case("rts"));

    let mut baked_entries: Vec<(String, u32, u32, Vec<u32>)> = Vec::new();
    for (name, (color_opt, ao_opt)) in groups {
        let Some(color_path) = color_opt else {
            continue;
        };
        let c_raw = fs::read(&color_path)
            .map_err(|e| format!("Failed reading '{}': {}", color_path.display(), e))?;
        let color = decode_png_rgba(&c_raw)?;
        let ao = if let Some(ap) = ao_opt {
            let a_raw = fs::read(&ap)
                .map_err(|e| format!("Failed reading '{}': {}", ap.display(), e))?;
            decode_png_rgba(&a_raw).ok()
        } else {
            None
        };

        let count = (color.width as usize) * (color.height as usize);
        let mut pixels = Vec::with_capacity(count);
        for i in 0..count {
            let r = color.rgba[i * 4] as f32;
            let g = color.rgba[i * 4 + 1] as f32;
            let b = color.rgba[i * 4 + 2] as f32;
            let a = color.rgba[i * 4 + 3] as u32;
            let ao_f = if let Some(ref ao_img) = ao {
                let ax = (i % color.width as usize).min(ao_img.width as usize - 1);
                let ay = (i / color.width as usize).min(ao_img.height as usize - 1);
                let ai = (ay * ao_img.width as usize + ax) * 4;
                0.35 + 0.65 * (ao_img.rgba[ai] as f32 / 255.0)
            } else {
                1.0
            };
            let br = (r * ao_f).round().clamp(0.0, 255.0) as u32;
            let bg = (g * ao_f).round().clamp(0.0, 255.0) as u32;
            let bb = (b * ao_f).round().clamp(0.0, 255.0) as u32;
            pixels.push((a << 24) | (br << 16) | (bg << 8) | bb);
        }
        baked_entries.push((name, color.width, color.height, pixels));
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let mut total_pixels = 0usize;
    let mut names = Vec::with_capacity(baked_entries.len());
    let bytes_written = if is_rts {
        let mut rts = String::from("module baked.textures;\n\n");
        for (name, w, h, pixels) in &baked_entries {
            total_pixels += pixels.len();
            names.push(name.clone());
            let id = sanitize_ident(name);
            rts.push_str(&format!("pub const {}_W: u32 = {};\n", id, w));
            rts.push_str(&format!("pub const {}_H: u32 = {};\n", id, h));
            rts.push_str(&format!("pub const {}_DATA: [u32; {}] = [", id, pixels.len()));
            for (idx, p) in pixels.iter().enumerate() {
                if idx > 0 {
                    rts.push_str(", ");
                }
                rts.push_str(&format!("0x{:08X}", p));
            }
            rts.push_str("];\n\n");
        }
        fs::write(output_path, rts.as_bytes())
            .map_err(|e| format!("Failed writing '{}': {}", output_path.display(), e))?;
        rts.len()
    } else {
        let mut bin = Vec::new();
        bin.extend_from_slice(&0x5854505Au32.to_le_bytes()); // "ZPTX"
        bin.extend_from_slice(&(baked_entries.len() as u32).to_le_bytes());
        for (name, w, h, pixels) in &baked_entries {
            total_pixels += pixels.len();
            names.push(name.clone());
            let nb = name.as_bytes();
            bin.extend_from_slice(&(nb.len() as u32).to_le_bytes());
            bin.extend_from_slice(nb);
            bin.extend_from_slice(&w.to_le_bytes());
            bin.extend_from_slice(&h.to_le_bytes());
            for p in pixels {
                bin.extend_from_slice(&p.to_le_bytes());
            }
        }
        fs::write(output_path, &bin)
            .map_err(|e| format!("Failed writing '{}': {}", output_path.display(), e))?;
        bin.len()
    };

    Ok(BakeTexturesReport {
        textures_baked: baked_entries.len(),
        pixels_total: total_pixels,
        bytes_written,
        names,
    })
}
