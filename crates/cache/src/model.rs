use crate::angle;
use crate::buffer::Buffer;
use crate::error::{Error, Result};
use crate::sequence::{AnimationClip, Frame, Framemap};

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
        let _animaya_flag = h.read_u8()?;
        let vertex_dx_len = h.read_u16()? as usize;
        let vertex_dy_len = h.read_u16()? as usize;
        let vertex_dz_len = h.read_u16()? as usize;
        let face_index_deltas_len = h.read_u16()? as usize;
        let texture_coords_len = h.read_u16()? as usize;
        let packed_groups_len = h.read_u16()? as usize;

        let mut cursor = num_texture_faces + vertex_count;
        let face_render_types_off = cursor;
        if face_render_types_flag == 1 {
            cursor += face_count;
        }
        let face_opcodes_off = cursor;
        cursor += face_count;
        let _priorities_off = cursor;
        if priority_raw == 255 {
            cursor += face_count;
        }
        let _packed_transparency_off = cursor;
        if packed_transparency_flag == 1 {
            cursor += face_count;
        }
        let packed_groups_off = cursor;
        cursor += packed_groups_len;
        let _transparencies_off = cursor;
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
        let _animaya_flag = h.read_u8()?;
        let vertex_dx_len = h.read_u16()? as usize;
        let vertex_dy_len = h.read_u16()? as usize;
        let vertex_dz_len = h.read_u16()? as usize;
        let face_index_deltas_len = h.read_u16()? as usize;
        let packed_groups_len = h.read_u16()? as usize;

        let mut cursor = vertex_count;
        let face_opcodes_off = cursor;
        cursor += face_count;
        let _priorities_off = cursor;
        if priority_raw == 255 {
            cursor += face_count;
        }
        let _packed_transparency_off = cursor;
        if packed_transparency_flag == 1 {
            cursor += face_count;
        }
        let render_texture_off = cursor;
        if render_flag == 1 {
            cursor += face_count;
        }
        let packed_groups_off = cursor;
        cursor += packed_groups_len;
        let _transparencies_off = cursor;
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

    /// Animate the model with a sequence clip and return cache-space positions.
    /// The caller converts Y to engine space (engine Y is up; cache Y is negative
    /// up).
    pub fn animated_positions(&self, clip: &AnimationClip, time_ms: f32) -> Vec<[f32; 3]> {
        self.animated_positions_impl(clip, time_ms, None)
    }

    /// Animate while ignoring local transforms for `masked_groups`. Root and
    /// other whole-body transforms still move those vertices, which lets an
    /// arm keep following the torso while its own swing is frozen.
    pub fn animated_positions_masked(
        &self,
        clip: &AnimationClip,
        time_ms: f32,
        masked_groups: &[u8],
    ) -> Vec<[f32; 3]> {
        self.animated_positions_impl(clip, time_ms, Some(masked_groups))
    }

    fn animated_positions_impl(
        &self,
        clip: &AnimationClip,
        time_ms: f32,
        masked_groups: Option<&[u8]>,
    ) -> Vec<[f32; 3]> {
        let original = self
            .vertex_x
            .iter()
            .zip(&self.vertex_y)
            .zip(&self.vertex_z)
            .map(|((&x, &y), &z)| [x, y, z])
            .collect::<Vec<_>>();

        if clip.frames.is_empty() || clip.sequence.frame_lengths.is_empty() {
            return original
                .into_iter()
                .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
                .collect();
        }

        let total_ticks: u32 = clip
            .sequence
            .frame_lengths
            .iter()
            .map(|&l| l.max(1) as u32)
            .sum();
        let loop_ms = total_ticks as f32 * 20.0;
        let time = if clip.sequence.max_loops > 1 {
            time_ms.rem_euclid(loop_ms.max(1.0))
        } else {
            time_ms.clamp(0.0, loop_ms)
        };

        let mut elapsed = 0.0f32;
        let mut frame_index = 0usize;
        let mut frame_progress = 0.0f32;
        for (i, &length) in clip.sequence.frame_lengths.iter().enumerate() {
            let length_ms = length.max(1) as f32 * 20.0;
            if time < elapsed + length_ms || i + 1 == clip.sequence.frame_lengths.len() {
                frame_index = i;
                frame_progress = ((time - elapsed) / length_ms).clamp(0.0, 1.0);
                break;
            }
            elapsed += length_ms;
        }

        let next_index = (frame_index + 1) % clip.frames.len();
        let frame_a = &clip.frames[frame_index.min(clip.frames.len() - 1)];
        let framemap_a = &clip.framemaps[frame_index.min(clip.framemaps.len() - 1)];
        let frame_b = &clip.frames[next_index.min(clip.frames.len() - 1)];
        let framemap_b = &clip.framemaps[next_index.min(clip.framemaps.len() - 1)];

        let mut a = original.clone();
        let mut b = original;
        self.apply_frame(frame_a, framemap_a, &mut a, masked_groups);
        self.apply_frame(frame_b, framemap_b, &mut b, masked_groups);

        a.iter()
            .zip(&b)
            .map(|(p, q)| {
                [
                    p[0] as f32 + (q[0] - p[0]) as f32 * frame_progress,
                    p[1] as f32 + (q[1] - p[1]) as f32 * frame_progress,
                    p[2] as f32 + (q[2] - p[2]) as f32 * frame_progress,
                ]
            })
            .collect()
    }

    /// Apply an NPC palette swap. Values are cache HSL palette indices; the
    /// first matching `find` wins for each face, matching the client's
    /// sequential recolor opcodes.
    pub fn recolor(&mut self, find: &[i16], replace: &[i16]) {
        for (&wanted, &replacement) in find.iter().zip(replace) {
            for color in &mut self.face_colors {
                if *color as i16 == wanted {
                    *color = replacement as u16;
                }
            }
        }
    }

    /// Combine several cache models into one mesh, the way multi-model NPCs
    /// are rendered by the OSRS client. Merging the vertex groups is required
    /// for animation: parts such as a crown or armour must share the body's
    /// group transforms instead of being animated as isolated meshes.
    pub fn merge(parts: &[Model]) -> Model {
        let mut out = Model {
            id: parts.first().map(|part| part.id).unwrap_or(0),
            vertex_count: 0,
            face_count: 0,
            num_texture_faces: 0,
            vertex_x: Vec::new(),
            vertex_y: Vec::new(),
            vertex_z: Vec::new(),
            faces: Vec::new(),
            face_colors: Vec::new(),
            face_render_types: Vec::new(),
            face_textures: None,
            texture_coords: None,
            face_normals: Vec::new(),
            vertex_groups: None,
            priority: parts.iter().map(|part| part.priority).max().unwrap_or(0),
        };
        let any_textures = parts.iter().any(|part| part.face_textures.is_some());
        let any_coords = parts.iter().any(|part| part.texture_coords.is_some());
        if any_textures {
            out.face_textures = Some(Vec::new());
        }
        if any_coords {
            out.texture_coords = Some(Vec::new());
        }

        for part in parts {
            let base = out.vertex_x.len() as u32;
            out.vertex_x.extend_from_slice(&part.vertex_x);
            out.vertex_y.extend_from_slice(&part.vertex_y);
            out.vertex_z.extend_from_slice(&part.vertex_z);
            out.faces.extend(
                part.faces
                    .iter()
                    .map(|&[a, b, c]| [a + base, b + base, c + base]),
            );
            out.face_colors.extend_from_slice(&part.face_colors);
            out.face_render_types
                .extend_from_slice(&part.face_render_types);
            out.face_normals.extend_from_slice(&part.face_normals);
            out.num_texture_faces += part.num_texture_faces;

            if let Some(textures) = &mut out.face_textures {
                match &part.face_textures {
                    Some(part_textures) => textures.extend_from_slice(part_textures),
                    None => textures.extend(std::iter::repeat_n(-1, part.face_count)),
                }
            }
            if let Some(coords) = &mut out.texture_coords {
                match &part.texture_coords {
                    Some(part_coords) => coords.extend_from_slice(part_coords),
                    None => coords.extend(std::iter::repeat_n(-1, part.face_count)),
                }
            }

            if let Some(groups) = &part.vertex_groups {
                let merged = out.vertex_groups.get_or_insert_with(Vec::new);
                if merged.len() < groups.len() {
                    merged.resize(groups.len(), Vec::new());
                }
                for (index, vertices) in groups.iter().enumerate() {
                    merged[index].extend(vertices.iter().map(|&vertex| vertex + base));
                }
            }
        }

        out.vertex_count = out.vertex_x.len();
        out.face_count = out.faces.len();
        out
    }

    fn apply_frame(
        &self,
        frame: &Frame,
        framemap: &Framemap,
        positions: &mut [[i32; 3]],
        masked_groups: Option<&[u8]>,
    ) {
        let mut offset = [0i32; 3];

        for (entry, &group_index) in frame.index_frame_ids.iter().enumerate() {
            let group_index = group_index as usize;
            if group_index >= framemap.types.len() {
                continue;
            }
            let group_type = framemap.types[group_index];
            let group_ids = &framemap.frame_maps[group_index];
            if group_ids.is_empty() {
                continue;
            }
            let dx = frame.translator_x.get(entry).copied().unwrap_or(0);
            let dy = frame.translator_y.get(entry).copied().unwrap_or(0);
            let dz = frame.translator_z.get(entry).copied().unwrap_or(0);

            // Whole-body transforms (root, torso rotation) must keep moving
            // the masked limbs; only local joint transforms are skipped.
            let global = self
                .vertex_groups
                .as_ref()
                .is_some_and(|groups| group_ids.len() * 2 >= groups.len());
            let mut vertices = Vec::new();
            for &group in group_ids {
                if !global && masked_groups.is_some_and(|masked| masked.contains(&group)) {
                    continue;
                }
                if let Some(group_vertices) = self
                    .vertex_groups
                    .as_ref()
                    .and_then(|g| g.get(group as usize))
                {
                    vertices.extend_from_slice(group_vertices);
                }
            }
            if vertices.is_empty() {
                continue;
            }

            match group_type {
                0 => {
                    let mut sum = [0i64; 3];
                    for &vertex in &vertices {
                        sum[0] += positions[vertex as usize][0] as i64;
                        sum[1] += positions[vertex as usize][1] as i64;
                        sum[2] += positions[vertex as usize][2] as i64;
                    }
                    let count = vertices.len() as i64;
                    offset = [
                        dx + (sum[0] / count) as i32,
                        dy + (sum[1] / count) as i32,
                        dz + (sum[2] / count) as i32,
                    ];
                }
                1 => {
                    for &vertex in &vertices {
                        let p = &mut positions[vertex as usize];
                        p[0] = p[0].saturating_add(dx);
                        p[1] = p[1].saturating_add(dy);
                        p[2] = p[2].saturating_add(dz);
                    }
                }
                2 => {
                    let ax = (dx & 255) * 8;
                    let ay = (dy & 255) * 8;
                    let az = (dz & 255) * 8;
                    for &vertex in &vertices {
                        let p = &mut positions[vertex as usize];
                        p[0] = p[0].saturating_sub(offset[0]);
                        p[1] = p[1].saturating_sub(offset[1]);
                        p[2] = p[2].saturating_sub(offset[2]);

                        if az != 0 {
                            let sin = angle::sine(az) as i64;
                            let cos = angle::cosine(az) as i64;
                            let x = ((sin * p[1] as i64 + cos * p[0] as i64) >> 16) as i32;
                            let y = ((cos * p[1] as i64 - sin * p[0] as i64) >> 16) as i32;
                            p[0] = x;
                            p[1] = y;
                        }
                        if ax != 0 {
                            let sin = angle::sine(ax) as i64;
                            let cos = angle::cosine(ax) as i64;
                            let y = ((cos * p[1] as i64 - sin * p[2] as i64) >> 16) as i32;
                            let z = ((sin * p[1] as i64 + cos * p[2] as i64) >> 16) as i32;
                            p[1] = y;
                            p[2] = z;
                        }
                        if ay != 0 {
                            let sin = angle::sine(ay) as i64;
                            let cos = angle::cosine(ay) as i64;
                            let z = ((sin * p[2] as i64 + cos * p[0] as i64) >> 16) as i32;
                            let x = ((cos * p[2] as i64 - sin * p[0] as i64) >> 16) as i32;
                            p[2] = z;
                            p[0] = x;
                        }

                        p[0] = p[0].saturating_add(offset[0]);
                        p[1] = p[1].saturating_add(offset[1]);
                        p[2] = p[2].saturating_add(offset[2]);
                    }
                }
                3 => {
                    for &vertex in &vertices {
                        let p = &mut positions[vertex as usize];
                        p[0] = p[0].saturating_sub(offset[0]);
                        p[1] = p[1].saturating_sub(offset[1]);
                        p[2] = p[2].saturating_sub(offset[2]);
                        p[0] = (dx * p[0]) / 128;
                        p[1] = (dy * p[1]) / 128;
                        p[2] = (dz * p[2]) / 128;
                        p[0] = p[0].saturating_add(offset[0]);
                        p[1] = p[1].saturating_add(offset[1]);
                        p[2] = p[2].saturating_add(offset[2]);
                    }
                }
                _ => {}
            }
        }
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
                std::mem::swap(&mut v1, &mut v2);
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

    fn tiny_model(id: u32, vertices: [[i32; 3]; 3], group: u8) -> Model {
        let mut groups = vec![Vec::new(); group as usize + 1];
        groups[group as usize] = vec![0, 1, 2];
        Model {
            id,
            vertex_count: 3,
            face_count: 1,
            num_texture_faces: 0,
            vertex_x: vertices.iter().map(|v| v[0]).collect(),
            vertex_y: vertices.iter().map(|v| v[1]).collect(),
            vertex_z: vertices.iter().map(|v| v[2]).collect(),
            faces: vec![[0, 1, 2]],
            face_colors: vec![20],
            face_render_types: Vec::new(),
            face_textures: None,
            texture_coords: None,
            face_normals: vec![[0.0, 1.0, 0.0]],
            vertex_groups: Some(groups),
            priority: 0,
        }
    }

    #[test]
    fn merge_offsets_faces_and_vertex_groups() {
        let body = tiny_model(1, [[0, 0, 0], [1, 0, 0], [0, 1, 0]], 2);
        let crown = tiny_model(2, [[0, 0, 1], [1, 0, 1], [0, 1, 1]], 1);

        let merged = Model::merge(&[body, crown]);

        assert_eq!(merged.vertex_count, 6);
        assert_eq!(merged.face_count, 2);
        assert_eq!(merged.faces, vec![[0, 1, 2], [3, 4, 5]]);
        assert_eq!(merged.vertex_groups.as_ref().unwrap()[2], vec![0, 1, 2]);
        assert_eq!(merged.vertex_groups.as_ref().unwrap()[1], vec![3, 4, 5]);
    }

    #[test]
    fn recolor_swaps_matching_palette_indices_in_order() {
        let mut model = tiny_model(1, [[0, 0, 0], [1, 0, 0], [0, 1, 0]], 2);
        model.face_colors = vec![20, 30, 40];
        model.recolor(&[20, 30], &[500, 600]);
        // 500 is itself a find value in the second pass, exercising the
        // sequential opcode behaviour.
        model.recolor(&[500], &[700]);
        assert_eq!(model.face_colors, vec![700, 600, 40]);
    }
}
