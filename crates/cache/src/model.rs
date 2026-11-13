use crate::buffer::Buffer;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub id: u32,
    pub vertex_count: usize,
    pub face_count: usize,
    pub num_texture_faces: usize,
    pub vertex_x: Vec<i32>,
    pub vertex_y: Vec<i32>,
    pub vertex_z: Vec<i32>,
    pub faces: Vec<[u32; 3]>,
    pub face_colors: Vec<u16>,
    pub face_render_types: Vec<u8>,
    pub face_textures: Option<Vec<i16>>,
    pub texture_coords: Option<Vec<i8>>,
    pub face_normals: Vec<[f32; 3]>,
    pub vertex_groups: Option<Vec<Vec<u32>>>,
    pub priority: i8,
}

impl Model {
    pub fn load(id: u32, data: &[u8]) -> Result<Self> {
        if data.len() < 2 {
            return Err(Error::parse(format!("model {id}: too short")));
        }

        let tail = &data[data.len() - 2..];
        match tail {
            [0xff, 0xfd] => Self::decode_type3(id, data),
            [0xff, 0xfe] => Self::decode_type2(id, data),
            [0xff, 0xff] => Err(Error::parse(format!(
                "model {id}: type 1 not implemented yet"
            ))),
            _ => Err(Error::parse(format!(
                "model {id}: old format not implemented yet"
            ))),
        }
    }

    fn decode_type3(id: u32, data: &[u8]) -> Result<Self> {
        if data.len() < 26 {
            return Err(Error::parse(format!("model {id}: type 3 too short")));
        }

        let mut h = Buffer::new(&data[data.len() - 26..]);
        let vertex_count = h.read_u16()? as usize;
        let face_count = h.read_u16()? as usize;
        let num_texture_faces = h.read_u8()? as usize;
        let face_render_types_flag = h.read_u8()?;
        let priority_raw = h.read_u8()?;
        let face_transparencies_flag = h.read_u8()?;
        let packed_transparency_flag = h.read_u8()?;
        let face_textures_flag = h.read_u8()?;
        let packed_groups_flag = h.read_u8()?;
        let animaya_flag = h.read_u8()?;
        let vertex_dx_len = h.read_u16()? as usize;
        let vertex_dy_len = h.read_u16()? as usize;
        let vertex_dz_len = h.read_u16()? as usize;
        let face_index_deltas_len = h.read_u16()? as usize;
        let texture_coords_len = h.read_u16()? as usize;
        let packed_groups_len = h.read_u16()? as usize;

        let mut tex_types = vec![0u8; num_texture_faces];
        if num_texture_faces > 0 {
            tex_types.copy_from_slice(&data[..num_texture_faces]);
        }
        let mut tex_type0 = 0usize;
        let mut tex_type123 = 0usize;
        let mut tex_type2 = 0usize;
        for &t in &tex_types {
            if t == 0 {
                tex_type0 += 1;
            } else if (1..=3).contains(&t) {
                tex_type123 += 1;
            }
            if t == 2 {
                tex_type2 += 1;
            }
        }

        let mut cursor = num_texture_faces + vertex_count;
        let face_render_types_off = cursor;
        if face_render_types_flag == 1 {
            cursor += face_count;
        }
        let face_opcodes_off = cursor;
        cursor += face_count;
        let priorities_off = cursor;
        if priority_raw == 255 {
            cursor += face_count;
        }
        let packed_transparency_off = cursor;
        if packed_transparency_flag == 1 {
            cursor += face_count;
        }
        let packed_groups_off = cursor;
        cursor += packed_groups_len;
        let transparencies_off = cursor;
        if face_transparencies_flag == 1 {
            cursor += face_count;
        }
        let face_deltas_off = cursor;
        cursor += face_index_deltas_len;
        let face_textures_off = cursor;
        if face_textures_flag == 1 {
            cursor += face_count * 2;
        }
        let texture_coords_off = cursor;
        cursor += texture_coords_len;
        let face_colors_off = cursor;
        cursor += face_count * 2;
        let vertex_dx_off = cursor;
        cursor += vertex_dx_len;
        let vertex_dy_off = cursor;
        cursor += vertex_dy_len;
        let vertex_dz_off = cursor;
        cursor += vertex_dz_len;
        let tex_type0_off = cursor;
        cursor += tex_type0 * 6;
        let _tex_type123_off = cursor;
        cursor += tex_type123 * 6;
        cursor += tex_type123 * 6;
        cursor += tex_type123 * 2;
        cursor += tex_type123;
        cursor += tex_type123 * 2 + tex_type2 * 2;

        check_range(data.len(), vertex_dx_off, vertex_dx_len, id)?;
        check_range(data.len(), vertex_dy_off, vertex_dy_len, id)?;
        check_range(data.len(), vertex_dz_off, vertex_dz_len, id)?;
        check_range(data.len(), face_deltas_off, face_index_deltas_len, id)?;
        check_range(data.len(), face_colors_off, face_count * 2, id)?;
        check_range(
            data.len(),
            face_textures_off,
            if face_textures_flag == 1 {
                face_count * 2
            } else {
                0
            },
            id,
        )?;
        check_range(data.len(), texture_coords_off, texture_coords_len, id)?;

        let mut dx_buf = Buffer::new(&data[vertex_dx_off..vertex_dx_off + vertex_dx_len]);
        let mut dy_buf = Buffer::new(&data[vertex_dy_off..vertex_dy_off + vertex_dy_len]);
        let mut dz_buf = Buffer::new(&data[vertex_dz_off..vertex_dz_off + vertex_dz_len]);
        let mut deltas =
            Buffer::new(&data[face_deltas_off..face_deltas_off + face_index_deltas_len]);

        let mut vertex_x = vec![0i32; vertex_count];
        let mut vertex_y = vec![0i32; vertex_count];
        let mut vertex_z = vec![0i32; vertex_count];
        let mut packed_groups = if packed_groups_flag == 1 {
            Some(vec![0u8; vertex_count])
        } else {
            None
        };

        let (mut acc_x, mut acc_y, mut acc_z) = (0i32, 0i32, 0i32);
        for i in 0..vertex_count {
            let flags = data[num_texture_faces + i];
            let dx = if flags & 1 != 0 {
                dx_buf.read_short_smart()?
            } else {
                0
            };
            let dy = if flags & 2 != 0 {
                dy_buf.read_short_smart()?
            } else {
                0
            };
            let dz = if flags & 4 != 0 {
                dz_buf.read_short_smart()?
            } else {
                0
            };
            acc_x = acc_x.saturating_add(dx);
            acc_y = acc_y.saturating_add(dy);
            acc_z = acc_z.saturating_add(dz);
            vertex_x[i] = acc_x;
            vertex_y[i] = acc_y;
            vertex_z[i] = acc_z;
            if let Some(groups) = packed_groups.as_mut() {
                groups[i] = data[packed_groups_off + i];
            }
        }

        if animaya_flag == 1 {
            // Animaya data is not needed for static rendering. It follows the
            // packed group block and is only read by the client animation code.
        }

        let mut face_colors = Vec::with_capacity(face_count);
        for i in 0..face_count {
            let off = face_colors_off + i * 2;
            face_colors.push(u16::from_be_bytes([data[off], data[off + 1]]));
        }

        let mut face_render_types = vec![0u8; face_count];
        if face_render_types_flag == 1 {
            face_render_types
                .copy_from_slice(&data[face_render_types_off..face_render_types_off + face_count]);
        }

        let mut face_textures = if face_textures_flag == 1 {
            Some(Vec::with_capacity(face_count))
        } else {
            None
        };
        let mut texture_coords = if face_textures_flag == 1 && num_texture_faces > 0 {
            Some(Vec::with_capacity(face_count))
        } else {
            None
        };

        if let Some(textures) = face_textures.as_mut() {
            for i in 0..face_count {
                let off = face_textures_off + i * 2;
                textures.push(u16::from_be_bytes([data[off], data[off + 1]]) as i16 - 1);
            }
        }
        if let Some(coords) = texture_coords.as_mut() {
            for i in 0..face_count {
                coords.push((data[texture_coords_off + i] as i8) - 1);
            }
        }

        let faces = decode_faces(
            &mut deltas,
            &data[face_opcodes_off..face_opcodes_off + face_count],
            face_count,
            vertex_count,
            id,
        )?;
        let face_normals = compute_face_normals(&vertex_x, &vertex_y, &vertex_z, &faces);

        let vertex_groups = packed_groups.map(build_groups);
        let priority = if priority_raw == 255 {
            0
        } else {
            priority_raw as i8
        };

        Ok(Self {
            id,
            vertex_count,
            face_count,
            num_texture_faces,
            vertex_x,
            vertex_y,
            vertex_z,
            faces,
            face_colors,
            face_render_types,
            face_textures,
            texture_coords,
            face_normals,
            vertex_groups,
            priority,
        })
    }

    fn decode_type2(id: u32, data: &[u8]) -> Result<Self> {
        if data.len() < 23 {
            return Err(Error::parse(format!("model {id}: type 2 too short")));
        }

        let mut h = Buffer::new(&data[data.len() - 23..]);
        let vertex_count = h.read_u16()? as usize;
        let face_count = h.read_u16()? as usize;
        let num_texture_faces = h.read_u8()? as usize;
        let render_flag = h.read_u8()?;
        let priority_raw = h.read_u8()?;
        let transparencies_flag = h.read_u8()?;
        let packed_transparency_flag = h.read_u8()?;
        let packed_groups_flag = h.read_u8()?;
        let animaya_flag = h.read_u8()?;
        let vertex_dx_len = h.read_u16()? as usize;
        let vertex_dy_len = h.read_u16()? as usize;
        let vertex_dz_len = h.read_u16()? as usize;
        let face_index_deltas_len = h.read_u16()? as usize;
        let packed_groups_len = h.read_u16()? as usize;

        let mut cursor = vertex_count;
        let face_opcodes_off = cursor;
        cursor += face_count;
        let priorities_off = cursor;
        if priority_raw == 255 {
            cursor += face_count;
        }
        let packed_transparency_off = cursor;
        if packed_transparency_flag == 1 {
            cursor += face_count;
        }
        let render_texture_off = cursor;
        if render_flag == 1 {
            cursor += face_count;
        }
        let packed_groups_off = cursor;
        cursor += packed_groups_len;
        let transparencies_off = cursor;
        if transparencies_flag == 1 {
            cursor += face_count;
        }
        let face_deltas_off = cursor;
        cursor += face_index_deltas_len;
        let face_colors_off = cursor;
        cursor += face_count * 2;
        let tex_indices_off = cursor;
        cursor += num_texture_faces * 6;
        let vertex_dx_off = cursor;
        cursor += vertex_dx_len;
        let vertex_dy_off = cursor;
        cursor += vertex_dy_len;
        let vertex_dz_off = cursor;
        cursor += vertex_dz_len;

        check_range(data.len(), vertex_dx_off, vertex_dx_len, id)?;
        check_range(data.len(), vertex_dy_off, vertex_dy_len, id)?;
        check_range(data.len(), vertex_dz_off, vertex_dz_len, id)?;
        check_range(data.len(), face_deltas_off, face_index_deltas_len, id)?;
        check_range(data.len(), face_colors_off, face_count * 2, id)?;
        check_range(data.len(), tex_indices_off, num_texture_faces * 6, id)?;

        let mut dx_buf = Buffer::new(&data[vertex_dx_off..vertex_dx_off + vertex_dx_len]);
        let mut dy_buf = Buffer::new(&data[vertex_dy_off..vertex_dy_off + vertex_dy_len]);
        let mut dz_buf = Buffer::new(&data[vertex_dz_off..vertex_dz_off + vertex_dz_len]);
        let mut deltas =
            Buffer::new(&data[face_deltas_off..face_deltas_off + face_index_deltas_len]);

        let mut vertex_x = vec![0i32; vertex_count];
        let mut vertex_y = vec![0i32; vertex_count];
        let mut vertex_z = vec![0i32; vertex_count];
        let mut packed_groups = if packed_groups_flag == 1 {
            Some(vec![0u8; vertex_count])
        } else {
            None
        };

        let (mut acc_x, mut acc_y, mut acc_z) = (0i32, 0i32, 0i32);
        for i in 0..vertex_count {
            let flags = data[i];
            let dx = if flags & 1 != 0 {
                dx_buf.read_short_smart()?
            } else {
                0
            };
            let dy = if flags & 2 != 0 {
                dy_buf.read_short_smart()?
            } else {
                0
            };
            let dz = if flags & 4 != 0 {
                dz_buf.read_short_smart()?
            } else {
                0
            };
            acc_x = acc_x.saturating_add(dx);
            acc_y = acc_y.saturating_add(dy);
            acc_z = acc_z.saturating_add(dz);
            vertex_x[i] = acc_x;
            vertex_y[i] = acc_y;
            vertex_z[i] = acc_z;
            if let Some(groups) = packed_groups.as_mut() {
                groups[i] = data[packed_groups_off + i];
            }
        }

        let mut face_colors = Vec::with_capacity(face_count);
        for i in 0..face_count {
            let off = face_colors_off + i * 2;
            face_colors.push(u16::from_be_bytes([data[off], data[off + 1]]));
        }

        let mut face_render_types = vec![0u8; face_count];
        let mut face_textures = vec![-1i16; face_count];
        let mut texture_coords = vec![-1i8; face_count];
        let mut any_textures = false;
        let mut any_render_types = false;

        if render_flag == 1 {
            for i in 0..face_count {
                let v = data[render_texture_off + i];
                if v & 1 == 1 {
                    face_render_types[i] = 1;
                    any_render_types = true;
                }
                if v & 2 == 2 {
                    texture_coords[i] = (v >> 2) as i8;
                    face_textures[i] = face_colors[i] as i16;
                    face_colors[i] = 127;
                    any_textures = true;
                }
            }
        }

        let faces = decode_faces(
            &mut deltas,
            &data[face_opcodes_off..face_opcodes_off + face_count],
            face_count,
            vertex_count,
            id,
        )?;
        let face_normals = compute_face_normals(&vertex_x, &vertex_y, &vertex_z, &faces);

        if !any_render_types {
            face_render_types.clear();
        }

        let vertex_groups = packed_groups.map(build_groups);
        let priority = if priority_raw == 255 {
            0
        } else {
            priority_raw as i8
        };

        Ok(Self {
            id,
            vertex_count,
            face_count,
            num_texture_faces,
            vertex_x,
            vertex_y,
            vertex_z,
            faces,
            face_colors,
            face_render_types,
            face_textures: if any_textures {
                Some(face_textures)
            } else {
                None
            },
            texture_coords: if any_textures {
                Some(texture_coords)
            } else {
                None
            },
            face_normals,
            vertex_groups,
            priority,
        })
    }

    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        if self.vertex_count == 0 {
            return ([0; 3], [0; 3]);
        }
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for i in 0..self.vertex_count {
            min[0] = min[0].min(self.vertex_x[i]);
            min[1] = min[1].min(self.vertex_y[i]);
            min[2] = min[2].min(self.vertex_z[i]);
            max[0] = max[0].max(self.vertex_x[i]);
            max[1] = max[1].max(self.vertex_y[i]);
            max[2] = max[2].max(self.vertex_z[i]);
        }
        (min, max)
    }

    pub fn face_rgb(&self, face: usize, brightness: f32) -> [u8; 3] {
        let hsl = self.face_colors.get(face).copied().unwrap_or(0);
        hsl_to_rgb(hsl, brightness)
    }
}

fn check_range(len: usize, off: usize, size: usize, id: u32) -> Result<()> {
    if off.checked_add(size).is_none_or(|end| end > len) {
        return Err(Error::parse(format!(
            "model {id}: block {off}..{} outside {len} bytes",
            off.saturating_add(size)
        )));
    }
    Ok(())
}

fn decode_faces(
    deltas: &mut Buffer<'_>,
    opcodes: &[u8],
    face_count: usize,
    vertex_count: usize,
    id: u32,
) -> Result<Vec<[u32; 3]>> {
    let mut faces = vec![[0u32; 3]; face_count];
    let mut v1 = 0i32;
    let mut v2 = 0i32;
    let mut v3 = 0i32;
    let mut last = 0i32;

    for i in 0..face_count {
        match opcodes[i] {
            1 => {
                v1 = deltas.read_short_smart()?.saturating_add(last);
                v2 = deltas.read_short_smart()?.saturating_add(v1);
                v3 = deltas.read_short_smart()?.saturating_add(v2);
                last = v3;
                faces[i] = [v1 as u32, v2 as u32, v3 as u32];
            }
            2 => {
                v2 = v3;
                v3 = deltas.read_short_smart()?.saturating_add(last);
                last = v3;
                faces[i] = [v1 as u32, v2 as u32, v3 as u32];
            }
            3 => {
                v1 = v3;
                v3 = deltas.read_short_smart()?.saturating_add(last);
                last = v3;
                faces[i] = [v1 as u32, v2 as u32, v3 as u32];
            }
            4 => {
                let old_v1 = v1;
                v1 = v2;
                v2 = old_v1;
                v3 = deltas.read_short_smart()?.saturating_add(last);
                last = v3;
                faces[i] = [v1 as u32, old_v1 as u32, v3 as u32];
            }
            _ => {}
        }

        let f = faces[i];
        if f[0] as usize >= vertex_count
            || f[1] as usize >= vertex_count
            || f[2] as usize >= vertex_count
        {
            return Err(Error::parse(format!(
                "model {id}: face {i} references out-of-range vertices {f:?} (count {vertex_count})"
            )));
        }
    }

    Ok(faces)
}

fn compute_face_normals(xs: &[i32], ys: &[i32], zs: &[i32], faces: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut normals = Vec::with_capacity(faces.len());
    for &[a, b, c] in faces {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        let e1 = [
            (xs[b] - xs[a]) as f32,
            (ys[b] - ys[a]) as f32,
            (zs[b] - zs[a]) as f32,
        ];
        let e2 = [
            (xs[c] - xs[a]) as f32,
            (ys[c] - ys[a]) as f32,
            (zs[c] - zs[a]) as f32,
        ];
        let mut n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 1e-6 {
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
        } else {
            n = [0.0, 1.0, 0.0];
        }
        normals.push(n);
    }
    normals
}

fn build_groups(packed: Vec<u8>) -> Vec<Vec<u32>> {
    let max_group = packed.iter().copied().max().unwrap_or(0) as usize;
    let mut groups = vec![Vec::new(); max_group + 1];
    for (vertex, &group) in packed.iter().enumerate() {
        groups[group as usize].push(vertex as u32);
    }
    groups
}

/// Jagex's HSL palette conversion. `brightness` is an exponent: 0.6 is bright,
/// 0.9 is dark.
pub fn hsl_to_rgb(hsl: u16, brightness: f32) -> [u8; 3] {
    const HUE_OFFSET: f32 = 0.5 / 64.0;
    const SATURATION_OFFSET: f32 = 0.5 / 8.0;

    let hue = (((hsl >> 10) & 63) as f32) / 64.0 + HUE_OFFSET;
    let saturation = (((hsl >> 7) & 7) as f32) / 8.0 + SATURATION_OFFSET;
    let luminance = ((hsl & 127) as f32) / 128.0;

    let chroma = (1.0 - (2.0 * luminance - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue * 6.0) % 2.0 - 1.0).abs());
    let lightness = luminance - chroma / 2.0;

    let (mut r, mut g, mut b) = (lightness, lightness, lightness);
    match (hue * 6.0) as i32 {
        0 => {
            r += chroma;
            g += x;
        }
        1 => {
            g += chroma;
            r += x;
        }
        2 => {
            g += chroma;
            b += x;
        }
        3 => {
            b += chroma;
            g += x;
        }
        4 => {
            b += chroma;
            r += x;
        }
        _ => {
            r += chroma;
            b += x;
        }
    }

    let adjust = |v: f32| -> u8 {
        let v = v.clamp(0.0, 1.0).powf(brightness);
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };

    let rgb = [adjust(r), adjust(g), adjust(b)];
    if rgb == [0, 0, 0] { [1, 1, 1] } else { rgb }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_is_in_range() {
        for hsl in [0u16, 1, 1024, 2048, 4096, 65535] {
            let rgb = hsl_to_rgb(hsl, 0.7);
            assert!(rgb.iter().all(|&v| v > 0));
        }
    }
}
