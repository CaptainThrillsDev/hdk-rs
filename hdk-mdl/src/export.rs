//! Export-related model structures and conversion logic
//!
//! This module defines export-friendly representations of the model data,
//! suitable for serialization to JSON. It includes methods to convert from
//! the raw binary model structures to these export formats.
//!
//! This is only enabled when the `export` feature is activated.

use crate::{Element, Model};
use serde::Serialize;

/// Export-friendly model representation used for JSON output.
#[derive(Serialize)]
pub struct ExportModel {
    pub skeleton_key: u32,
    pub joint_count: u32,
    pub elements: Vec<ExportElement>,
    pub bounds: [f32; 4],
}

/// Export-friendly element representation used for JSON output.
#[derive(Serialize)]
pub struct ExportElement {
    pub elem_size: u32,
    pub unk0: u32,
    pub name_hash: u32,
    pub primitive_type: u32,
    pub mat_index: u32,
    pub flags: u32,
    pub stream_ofs: u32,
    pub index_ofs: u32,
    pub vertex_ofs: u32,
    pub mesh: Option<ExportMesh>,
}

/// Export-friendly mesh representation used for JSON output.
#[derive(Serialize)]
pub struct ExportMesh {
    pub num_faces: u32,
    pub num_vct: u32,
    pub vertex_stride: u32,
    pub material_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positions: Option<Vec<[f32; 3]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u16>>,
}

impl Model {
    /// Converts the raw binary model into the friendly JSON-export struct
    pub fn to_export(&self) -> ExportModel {
        ExportModel {
            skeleton_key: self.skeleton_key,
            joint_count: self.joint_count,
            bounds: self.bounds,
            elements: self
                .elements
                .iter()
                .map(|e| e.to_export_element())
                .collect(),
        }
    }
}

impl Element {
    /// Convenience to get indices as u16 (Big Endian)
    pub fn get_indices(&self) -> Option<Vec<u16>> {
        let raw = self.indices.0.as_ref()?;
        raw.chunks_exact(2)
            .map(|c| Some(u16::from_be_bytes([c[0], c[1]])))
            .collect()
    }

    /// Convenience to get positions (assuming stride >= 12 and f32x3 at offset 0)
    pub fn get_positions(&self) -> Option<Vec<[f32; 3]>> {
        let raw = self.vertices.0.as_ref()?;
        let stride = self.vertex_stride as usize;
        if stride < 12 {
            return None;
        }

        (0..self.num_vct as usize)
            .map(|i| {
                let start = i * stride;
                let b = &raw[start..start + 12];
                Some([
                    f32::from_be_bytes(b[0..4].try_into().ok()?),
                    f32::from_be_bytes(b[4..8].try_into().ok()?),
                    f32::from_be_bytes(b[8..12].try_into().ok()?),
                ])
            })
            .collect()
    }

    /// Converts to the friendly JSON-export struct
    pub fn to_export_element(&self) -> ExportElement {
        let has_mesh = self.num_faces > 0 && self.num_vct > 0;

        let mesh = if has_mesh {
            Some(ExportMesh {
                num_faces: self.num_faces as u32,
                num_vct: self.num_vct as u32,
                vertex_stride: self.vertex_stride as u32,
                material_name: self.material_name.0.clone(),
                positions: self.get_positions(),
                indices: self.get_indices(),
            })
        } else {
            None
        };

        ExportElement {
            // Restore all the original derived fields
            elem_size: self.num_faces as u32,
            primitive_type: self.vertex_stride as u32,

            // Original logic: mat_index comes from v_offset
            mat_index: self.v_offset as u32,

            // Original logic: unk0 comes from f_offset
            unk0: self.f_offset as u32,

            // Original logic: name_hash comes from num_vct
            name_hash: self.num_vct as u32,

            flags: self.flags,
            stream_ofs: self.stream_ofs,
            index_ofs: self.m_offset1,
            vertex_ofs: self.vertex_ofs,

            mesh,
        }
    }
}
