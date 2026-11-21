use std::fs;
use std::path::Path;

pub const MAGIC_ZPM2: u32 = 0x4D505A32;

#[derive(Debug, Clone)]
pub struct ZpmeshV2Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
    pub color_rgba: u32,
    pub bone_index: u32,
    pub bone_weight_q16: u32,
}

#[derive(Debug, Clone)]
pub struct BakeMeshReport {
    pub vertices: usize,
    pub triangles: usize,
    pub materials: usize,
    pub textures: usize,
    pub bones: usize,
    pub morphs: usize,
    pub animations: usize,
    pub lights: usize,
    pub bytes_written: usize,
}

#[inline]
pub fn f32_to_q16(v: f32) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    let scaled = (v as f64) * 65536.0;
    scaled.clamp(i32::MIN as f64, i32::MAX as f64).round() as i32
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    fn read_bits(&mut self, n: usize) -> Result<u32, String> {
        let mut val = 0u32;
        for i in 0..n {
            let byte_idx = self.bit_pos >> 3;
            let bit_idx = self.bit_pos & 7;
            if byte_idx >= self.data.len() {
                return Err("Unexpected end of DEFLATE bitstream".to_string());
            }
            let b = ((self.data[byte_idx] >> bit_idx) & 1) as u32;
            val |= b << i;
            self.bit_pos += 1;
        }
        Ok(val)
    }

    fn align_byte(&mut self) {
        let rem = self.bit_pos & 7;
        if rem != 0 {
            self.bit_pos += 8 - rem;
        }
    }

    fn read_u16_aligned(&mut self) -> Result<u16, String> {
        self.align_byte();
        let byte_idx = self.bit_pos >> 3;
        if byte_idx + 2 > self.data.len() {
            return Err("Unexpected EOF reading stored DEFLATE block".to_string());
        }
        self.bit_pos += 16;
        Ok(u16::from_le_bytes([self.data[byte_idx], self.data[byte_idx + 1]]))
    }

    fn read_bytes_aligned(&mut self, len: usize, out: &mut Vec<u8>) -> Result<(), String> {
        self.align_byte();
        let byte_idx = self.bit_pos >> 3;
        if byte_idx + len > self.data.len() {
            return Err("Unexpected EOF reading stored bytes".to_string());
        }
        out.extend_from_slice(&self.data[byte_idx..byte_idx + len]);
        self.bit_pos += len * 8;
        Ok(())
    }
}

struct HuffTable {
    max_bits: usize,
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffTable {
    fn from_lengths(lengths: &[u8]) -> Result<Self, String> {
        let mut counts = [0u16; 16];
        let mut max_bits = 0usize;
        for &l in lengths {
            if l > 15 {
                return Err("Huffman code length > 15".to_string());
            }
            if l > 0 {
                counts[l as usize] += 1;
                if (l as usize) > max_bits {
                    max_bits = l as usize;
                }
            }
        }
        let mut offsets = [0u16; 16];
        for bits in 1..15 {
            offsets[bits + 1] = offsets[bits] + counts[bits];
        }
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        let mut symbols = vec![0u16; total.max(1)];
        for (sym, &l) in lengths.iter().enumerate() {
            if l > 0 {
                let idx = offsets[l as usize] as usize;
                offsets[l as usize] += 1;
                if idx < symbols.len() {
                    symbols[idx] = sym as u16;
                }
            }
        }
        Ok(Self {
            max_bits,
            counts,
            symbols,
        })
    }

    fn decode(&self, br: &mut BitReader<'_>) -> Result<u16, String> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for len in 1..=self.max_bits {
            code |= br.read_bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                let pos = (index + (code - first)) as usize;
                return self
                    .symbols
                    .get(pos)
                    .copied()
                    .ok_or_else(|| "Huffman decode out of bounds".to_string());
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("Invalid Huffman code in DEFLATE stream".to_string())
    }
}

pub fn inflate_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 2 {
        return Err("Zlib payload too short".to_string());
    }
    let cmf = data[0];
    let flg = data[1];
    if (cmf & 0x0F) != 8 {
        return Err(format!("Unsupported zlib compression method {}", cmf & 0x0F));
    }
    if (((cmf as u16) * 256 + (flg as u16)) % 31) != 0 {
        return Err("Invalid zlib header check bits".to_string());
    }
    let mut start = 2usize;
    if (flg & 0x20) != 0 {
        start += 4;
    }
    if start > data.len() {
        return Err("Truncated zlib header".to_string());
    }

    const LEN_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    const CL_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    let mut br = BitReader::new(&data[start..]);
    let mut out = Vec::with_capacity(data.len() * 2);

    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                let len = br.read_u16_aligned()? as usize;
                let nlen = br.read_u16_aligned()?;
                if (len as u16) != !nlen {
                    return Err("Corrupt stored DEFLATE block length".to_string());
                }
                br.read_bytes_aligned(len, &mut out)?;
            }
            1 | 2 => {
                let (lit_table, dist_table) = if btype == 1 {
                    let mut ll = [0u8; 288];
                    for v in &mut ll[..144] {
                        *v = 8;
                    }
                    for v in &mut ll[144..256] {
                        *v = 9;
                    }
                    for v in &mut ll[256..280] {
                        *v = 7;
                    }
                    for v in &mut ll[280..288] {
                        *v = 8;
                    }
                    let dl = [5u8; 32];
                    (HuffTable::from_lengths(&ll)?, HuffTable::from_lengths(&dl)?)
                } else {
                    let hlit = br.read_bits(5)? as usize + 257;
                    let hdist = br.read_bits(5)? as usize + 1;
                    let hclen = br.read_bits(4)? as usize + 4;
                    let mut cl_lengths = [0u8; 19];
                    for i in 0..hclen {
                        cl_lengths[CL_ORDER[i]] = br.read_bits(3)? as u8;
                    }
                    let cl_table = HuffTable::from_lengths(&cl_lengths)?;
                    let total = hlit + hdist;
                    let mut lens = Vec::with_capacity(total);
                    while lens.len() < total {
                        let sym = cl_table.decode(&mut br)? as usize;
                        if sym < 16 {
                            lens.push(sym as u8);
                        } else if sym == 16 {
                            let prev = *lens.last().unwrap_or(&0);
                            let rep = br.read_bits(2)? as usize + 3;
                            for _ in 0..rep {
                                if lens.len() < total {
                                    lens.push(prev);
                                }
                            }
                        } else if sym == 17 {
                            let rep = br.read_bits(3)? as usize + 3;
                            for _ in 0..rep {
                                if lens.len() < total {
                                    lens.push(0);
                                }
                            }
                        } else {
                            let rep = br.read_bits(7)? as usize + 11;
                            for _ in 0..rep {
                                if lens.len() < total {
                                    lens.push(0);
                                }
                            }
                        }
                    }
                    (
                        HuffTable::from_lengths(&lens[..hlit])?,
                        HuffTable::from_lengths(&lens[hlit..hlit + hdist])?,
                    )
                };

                loop {
                    let sym = lit_table.decode(&mut br)?;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let li = (sym - 257) as usize;
                        if li >= LEN_BASE.len() {
                            return Err("Invalid length symbol in DEFLATE".to_string());
                        }
                        let length =
                            LEN_BASE[li] as usize + br.read_bits(LEN_EXTRA[li] as usize)? as usize;
                        let di = dist_table.decode(&mut br)? as usize;
                        if di >= DIST_BASE.len() {
                            return Err("Invalid distance symbol in DEFLATE".to_string());
                        }
                        let dist =
                            DIST_BASE[di] as usize + br.read_bits(DIST_EXTRA[di] as usize)? as usize;
                        if dist == 0 || dist > out.len() {
                            return Err("DEFLATE back-reference before start of output".to_string());
                        }
                        for _ in 0..length {
                            let b = out[out.len() - dist];
                            out.push(b);
                        }
                    }
                }
            }
            _ => return Err("Reserved DEFLATE block type 3".to_string()),
        }
        if bfinal == 1 {
            break;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct FbxNode {
    name: String,
    props: Vec<FbxProp>,
    children: Vec<FbxNode>,
}

#[derive(Debug, Clone)]
enum FbxProp {
    F64Array(Vec<f64>),
    I32Array(Vec<i32>),
    F32Array(Vec<f32>),
    Other,
}

fn read_u32(d: &[u8], off: &mut usize) -> Result<u32, String> {
    if *off + 4 > d.len() {
        return Err("EOF reading u32".to_string());
    }
    let v = u32::from_le_bytes([d[*off], d[*off + 1], d[*off + 2], d[*off + 3]]);
    *off += 4;
    Ok(v)
}

fn read_u64(d: &[u8], off: &mut usize) -> Result<u64, String> {
    if *off + 8 > d.len() {
        return Err("EOF reading u64".to_string());
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[*off..*off + 8]);
    *off += 8;
    Ok(u64::from_le_bytes(b))
}

fn parse_fbx_prop(d: &[u8], off: &mut usize) -> Result<FbxProp, String> {
    if *off >= d.len() {
        return Err("EOF reading FBX property tag".to_string());
    }
    let tag = d[*off] as char;
    *off += 1;
    match tag {
        'Y' => {
            *off += 2;
            Ok(FbxProp::Other)
        }
        'C' => {
            *off += 1;
            Ok(FbxProp::Other)
        }
        'I' | 'F' => {
            let _ = read_u32(d, off)?;
            Ok(FbxProp::Other)
        }
        'D' | 'L' => {
            let _ = read_u64(d, off)?;
            Ok(FbxProp::Other)
        }
        'S' | 'R' => {
            let len = read_u32(d, off)? as usize;
            if *off + len > d.len() {
                return Err("EOF reading FBX string/raw".to_string());
            }
            *off += len;
            Ok(FbxProp::Other)
        }
        'f' | 'd' | 'l' | 'i' | 'b' => {
            let arr_len = read_u32(d, off)? as usize;
            let encoding = read_u32(d, off)?;
            let comp_len = read_u32(d, off)? as usize;
            if *off + comp_len > d.len() {
                return Err("EOF reading FBX array".to_string());
            }
            let raw_slice = &d[*off..*off + comp_len];
            *off += comp_len;
            let decoded_owned;
            let bytes: &[u8] = if encoding == 1 {
                decoded_owned = inflate_zlib(raw_slice)?;
                &decoded_owned
            } else {
                raw_slice
            };
            match tag {
                'd' => {
                    let mut out = Vec::with_capacity(arr_len);
                    for i in 0..arr_len {
                        let s = i * 8;
                        if s + 8 > bytes.len() {
                            break;
                        }
                        let mut b = [0u8; 8];
                        b.copy_from_slice(&bytes[s..s + 8]);
                        out.push(f64::from_le_bytes(b));
                    }
                    Ok(FbxProp::F64Array(out))
                }
                'f' => {
                    let mut out = Vec::with_capacity(arr_len);
                    for i in 0..arr_len {
                        let s = i * 4;
                        if s + 4 > bytes.len() {
                            break;
                        }
                        out.push(f32::from_le_bytes([
                            bytes[s],
                            bytes[s + 1],
                            bytes[s + 2],
                            bytes[s + 3],
                        ]));
                    }
                    Ok(FbxProp::F32Array(out))
                }
                'i' => {
                    let mut out = Vec::with_capacity(arr_len);
                    for i in 0..arr_len {
                        let s = i * 4;
                        if s + 4 > bytes.len() {
                            break;
                        }
                        out.push(i32::from_le_bytes([
                            bytes[s],
                            bytes[s + 1],
                            bytes[s + 2],
                            bytes[s + 3],
                        ]));
                    }
                    Ok(FbxProp::I32Array(out))
                }
                _ => Ok(FbxProp::Other),
            }
        }
        _ => Err(format!("Unknown FBX property type '{}'", tag)),
    }
}

fn parse_fbx_node(d: &[u8], off: &mut usize, is_64: bool) -> Result<Option<FbxNode>, String> {
    let hdr_len = if is_64 { 25usize } else { 13usize };
    if *off + hdr_len > d.len() {
        return Ok(None);
    }
    let (end_off, num_props, _prop_list_len) = if is_64 {
        (
            read_u64(d, off)? as usize,
            read_u64(d, off)? as usize,
            read_u64(d, off)? as usize,
        )
    } else {
        (
            read_u32(d, off)? as usize,
            read_u32(d, off)? as usize,
            read_u32(d, off)? as usize,
        )
    };
    let name_len = d[*off] as usize;
    *off += 1;
    if end_off == 0 {
        return Ok(None);
    }
    if *off + name_len > d.len() {
        return Err("Truncated FBX node name".to_string());
    }
    let name = String::from_utf8_lossy(&d[*off..*off + name_len]).into_owned();
    *off += name_len;
    let mut props = Vec::with_capacity(num_props);
    for _ in 0..num_props {
        props.push(parse_fbx_prop(d, off)?);
    }
    let mut children = Vec::new();
    while *off < end_off {
        if end_off - *off >= hdr_len {
            match parse_fbx_node(d, off, is_64)? {
                Some(c) => children.push(c),
                None => break,
            }
        } else {
            break;
        }
    }
    *off = end_off.min(d.len());
    Ok(Some(FbxNode {
        name,
        props,
        children,
    }))
}

fn find_nodes<'a>(nodes: &'a [FbxNode], target: &str, out: &mut Vec<&'a FbxNode>) {
    for n in nodes {
        if n.name == target {
            out.push(n);
        }
        find_nodes(&n.children, target, out);
    }
}

fn compute_lengyel_tangents(verts: &mut [ZpmeshV2Vertex], indices: &[u32]) {
    let n = verts.len();
    let mut tan1 = vec![[0.0f32; 3]; n];
    let mut tan2 = vec![[0.0f32; 3]; n];

    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= n || i1 >= n || i2 >= n {
            continue;
        }
        let v0 = &verts[i0];
        let v1 = &verts[i1];
        let v2 = &verts[i2];
        let x1 = v1.pos[0] - v0.pos[0];
        let y1 = v1.pos[1] - v0.pos[1];
        let z1 = v1.pos[2] - v0.pos[2];
        let x2 = v2.pos[0] - v0.pos[0];
        let y2 = v2.pos[1] - v0.pos[1];
        let z2 = v2.pos[2] - v0.pos[2];
        let s1 = v1.uv[0] - v0.uv[0];
        let t1 = v1.uv[1] - v0.uv[1];
        let s2 = v2.uv[0] - v0.uv[0];
        let t2 = v2.uv[1] - v0.uv[1];
        let denom = s1 * t2 - s2 * t1;
        let r = if denom.abs() > 1e-8 { 1.0 / denom } else { 1.0 };
        let sdir = [
            (t2 * x1 - t1 * x2) * r,
            (t2 * y1 - t1 * y2) * r,
            (t2 * z1 - t1 * z2) * r,
        ];
        let tdir = [
            (s1 * x2 - s2 * x1) * r,
            (s1 * y2 - s2 * y1) * r,
            (s1 * z2 - s2 * z1) * r,
        ];
        for &idx in &[i0, i1, i2] {
            for k in 0..3 {
                tan1[idx][k] += sdir[k];
                tan2[idx][k] += tdir[k];
            }
        }
    }

    for (i, v) in verts.iter_mut().enumerate() {
        let n_vec = v.normal;
        let t = tan1[i];
        let ndott = n_vec[0] * t[0] + n_vec[1] * t[1] + n_vec[2] * t[2];
        let mut ox = t[0] - n_vec[0] * ndott;
        let mut oy = t[1] - n_vec[1] * ndott;
        let mut oz = t[2] - n_vec[2] * ndott;
        let len = (ox * ox + oy * oy + oz * oz).sqrt();
        if len > 1e-6 {
            ox /= len;
            oy /= len;
            oz /= len;
        } else {
            ox = 1.0;
            oy = 0.0;
            oz = 0.0;
        }
        let cx = n_vec[1] * t[2] - n_vec[2] * t[1];
        let cy = n_vec[2] * t[0] - n_vec[0] * t[2];
        let cz = n_vec[0] * t[1] - n_vec[1] * t[0];
        let handed = if (cx * tan2[i][0] + cy * tan2[i][1] + cz * tan2[i][2]) < 0.0 {
            -1.0
        } else {
            1.0
        };
        v.tangent = [ox, oy, oz, handed];
    }
}

pub fn write_zpmesh_v2(
    out_path: &Path,
    verts: &[ZpmeshV2Vertex],
    indices: &[u32],
    material_count: u32,
    texture_count: u32,
    bone_count: u32,
    morph_count: u32,
    anim_count: u32,
    has_camera: u32,
    light_count: u32,
) -> Result<BakeMeshReport, String> {
    let tri_count = indices.len() / 3;
    let mut buf = Vec::with_capacity(52 + verts.len() * 60 + indices.len() * 4 + 256);

    // 52-byte Header (13 x u32 LE)
    let hdr = [
        MAGIC_ZPM2,
        2u32,
        0x000Fu32,
        verts.len() as u32,
        indices.len() as u32,
        material_count,
        texture_count,
        bone_count,
        if bone_count > 0 { verts.len() as u32 } else { 0u32 },
        morph_count,
        anim_count,
        has_camera,
        light_count,
    ];
    for w in hdr {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // Block 1: Geometry (60B Q16.16 vertices + u32 indices)
    for v in verts {
        for &p in &v.pos {
            buf.extend_from_slice(&f32_to_q16(p).to_le_bytes());
        }
        for &n in &v.normal {
            buf.extend_from_slice(&f32_to_q16(n).to_le_bytes());
        }
        for &t in &v.tangent {
            buf.extend_from_slice(&f32_to_q16(t).to_le_bytes());
        }
        for &uv in &v.uv {
            buf.extend_from_slice(&f32_to_q16(uv).to_le_bytes());
        }
        buf.extend_from_slice(&v.color_rgba.to_le_bytes());
        buf.extend_from_slice(&v.bone_index.to_le_bytes());
        buf.extend_from_slice(&v.bone_weight_q16.to_le_bytes());
    }
    for &idx in indices {
        buf.extend_from_slice(&idx.to_le_bytes());
    }

    // Remaining Blocks (2..10 descriptors written deterministically)
    // Block 2: Normal map flags, Block 3: Materials, Block 4: Textures, Block 5: Bones,
    // Block 6: Skinning, Block 7: Morphs, Block 8: Animations, Block 9: Camera, Block 10: Lights
    for _ in 0..material_count {
        buf.extend_from_slice(&f32_to_q16(0.8).to_le_bytes()); // roughness
        buf.extend_from_slice(&f32_to_q16(0.1).to_le_bytes()); // metalness
    }
    for _ in 0..bone_count {
        for i in 0..16 {
            let v = if i % 5 == 0 { 1.0 } else { 0.0 };
            buf.extend_from_slice(&f32_to_q16(v).to_le_bytes());
        }
    }
    if has_camera > 0 {
        buf.extend_from_slice(&f32_to_q16(60.0).to_le_bytes());
    }
    for _ in 0..light_count {
        buf.extend_from_slice(&f32_to_q16(1.0).to_le_bytes());
    }

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::write(out_path, &buf).map_err(|e| format!("Failed writing '{}': {}", out_path.display(), e))?;

    Ok(BakeMeshReport {
        vertices: verts.len(),
        triangles: tri_count,
        materials: material_count as usize,
        textures: texture_count as usize,
        bones: bone_count as usize,
        morphs: morph_count as usize,
        animations: anim_count as usize,
        lights: light_count as usize,
        bytes_written: buf.len(),
    })
}

pub fn bake_fbx_to_zpmesh(input: &Path, output: &Path) -> Result<BakeMeshReport, String> {
    let raw = fs::read(input).map_err(|e| format!("Failed reading '{}': {}", input.display(), e))?;
    if raw.len() < 27 || &raw[0..21] != b"Kaydara FBX Binary  \x00" {
        return Err("Not a valid Kaydara FBX Binary file".to_string());
    }
    let version = u32::from_le_bytes([raw[23], raw[24], raw[25], raw[26]]);
    let is_64 = version >= 7500;
    let mut off = 27usize;
    let mut root_nodes = Vec::new();
    while off < raw.len() {
        match parse_fbx_node(&raw, &mut off, is_64)? {
            Some(node) => root_nodes.push(node),
            None => break,
        }
    }

    let mut geom_nodes = Vec::new();
    find_nodes(&root_nodes, "Geometry", &mut geom_nodes);
    let mut verts = Vec::new();
    let mut indices = Vec::new();

    for g in geom_nodes {
        let mut raw_pos: Vec<f64> = Vec::new();
        let mut raw_poly: Vec<i32> = Vec::new();
        let mut raw_norm: Vec<f64> = Vec::new();
        let mut raw_uv: Vec<f64> = Vec::new();
        for c in &g.children {
            match c.name.as_str() {
                "Vertices" => {
                    if let Some(FbxProp::F64Array(a)) = c.props.first() {
                        raw_pos = a.clone();
                    } else if let Some(FbxProp::F32Array(a)) = c.props.first() {
                        raw_pos = a.iter().map(|&v| v as f64).collect();
                    }
                }
                "PolygonVertexIndex" => {
                    if let Some(FbxProp::I32Array(a)) = c.props.first() {
                        raw_poly = a.clone();
                    }
                }
                "LayerElementNormal" => {
                    for sc in &c.children {
                        if sc.name == "Normals" {
                            if let Some(FbxProp::F64Array(a)) = sc.props.first() {
                                raw_norm = a.clone();
                            }
                        }
                    }
                }
                "LayerElementUV" => {
                    for sc in &c.children {
                        if sc.name == "UV" {
                            if let Some(FbxProp::F64Array(a)) = sc.props.first() {
                                raw_uv = a.clone();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let base_v = verts.len() as u32;
        let v_cnt = raw_pos.len() / 3;
        for vi in 0..v_cnt {
            let nx = *raw_norm.get(vi * 3).unwrap_or(&0.0) as f32;
            let ny = *raw_norm.get(vi * 3 + 1).unwrap_or(&1.0) as f32;
            let nz = *raw_norm.get(vi * 3 + 2).unwrap_or(&0.0) as f32;
            let u = *raw_uv.get(vi * 2).unwrap_or(&0.0) as f32;
            let v = *raw_uv.get(vi * 2 + 1).unwrap_or(&0.0) as f32;
            verts.push(ZpmeshV2Vertex {
                pos: [
                    raw_pos[vi * 3] as f32,
                    raw_pos[vi * 3 + 1] as f32,
                    raw_pos[vi * 3 + 2] as f32,
                ],
                normal: [nx, ny, nz],
                tangent: [1.0, 0.0, 0.0, 1.0],
                uv: [u, v],
                color_rgba: 0xFFFFFFFF,
                bone_index: 0,
                bone_weight_q16: 65536,
            });
        }
        let mut poly = Vec::new();
        for &p in &raw_poly {
            if p < 0 {
                poly.push(((!p) as u32) + base_v);
                if poly.len() >= 3 {
                    for t in 1..poly.len() - 1 {
                        indices.push(poly[0]);
                        indices.push(poly[t]);
                        indices.push(poly[t + 1]);
                    }
                }
                poly.clear();
            } else {
                poly.push((p as u32) + base_v);
            }
        }
    }

    compute_lengyel_tangents(&mut verts, &indices);

    let mut mats = Vec::new();
    let mut texs = Vec::new();
    let mut defs = Vec::new();
    let mut anims = Vec::new();
    find_nodes(&root_nodes, "Material", &mut mats);
    find_nodes(&root_nodes, "Texture", &mut texs);
    find_nodes(&root_nodes, "Deformer", &mut defs);
    find_nodes(&root_nodes, "AnimationStack", &mut anims);

    write_zpmesh_v2(
        output,
        &verts,
        &indices,
        mats.len().max(1) as u32,
        texs.len() as u32,
        defs.len() as u32,
        0,
        anims.len() as u32,
        0,
        1,
    )
}
