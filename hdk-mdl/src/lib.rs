#[cfg(feature = "export")]
mod export;

use binrw::{binread, BinRead, NullString};
use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom};

#[cfg(feature = "export")]
use serde::{Deserialize, Serialize};
/// A pointer that stores a 32-bit signed offset relative to the *pointer's own position*.
///
/// It reads an `i32`. If the offset is valid (non-negative logic can be customized),
/// it seeks to `(pointer_pos + offset)`, reads `T`, and restores the position.
#[cfg_attr(feature = "export", derive(Serialize, Deserialize))]
pub struct RelPtr<T, const BIAS: i64 = 0>(pub Option<T>);

impl<T: BinRead + 'static, const BIAS: i64> BinRead for RelPtr<T, BIAS>
where
    T::Args<'static>: Clone,
{
    type Args<'a> = T::Args<'static>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let pos_before = reader.stream_position()?;
        let offset = <i32>::read_options(reader, endian, ())?;

        let return_pos = reader.stream_position()?; // always restore here

        // In your logic, negative offsets usually mean "null" or "invalid"
        if offset < 0 {
            return Ok(RelPtr(None));
        }

        // Calculate target absolute address: Position of ptr + offset
        let target_i64 = pos_before as i64 + offset as i64 + BIAS;
        if target_i64 < 0 {
            return Ok(RelPtr(None));
        }
        let target = target_i64 as u64;

        // Basic bounds check; if invalid, treat as null (matches old behavior).
        let file_len = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(return_pos))?;
        if target >= file_len {
            return Ok(RelPtr(None));
        }

        // Jump, try read, then restore.
        let value_res = (|| {
            reader.seek(SeekFrom::Start(target))?;
            T::read_options(reader, endian, args)
        })();

        reader.seek(SeekFrom::Start(return_pos))?;

        match value_res {
            Ok(value) => Ok(RelPtr(Some(value))),
            Err(_) => Ok(RelPtr(None)),
        }
    }
}

impl<T: Debug, const BIAS: i64> Debug for RelPtr<T, BIAS> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Handles the triple-indirection material lookup:
/// Offset1 -> Offset2 -> Offset3 -> String
#[cfg_attr(feature = "export", derive(Serialize, Deserialize))]
pub struct IndirectMaterial(pub Option<String>);

impl BinRead for IndirectMaterial {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        // The reference loader adds +0x20 to the first hop (mpos + off + 0x20).
        // The subsequent hops are plain relative pointers.
        type MatChain = RelPtr<RelPtr<RelPtr<NullString, 0>, 0>, 0x20>;
        let chain = MatChain::read_options(reader, endian, ())?;
        
        // Unpack the Russian nesting doll
        let str_opt = chain.0
            .and_then(|l2| l2.0)
            .and_then(|l3| l3.0)
            .map(|s| s.to_string());
            
        Ok(IndirectMaterial(str_opt))
    }
}

/// The main model structure representing the MDL file.
#[binread]
#[cfg_attr(feature = "export", derive(Serialize, Deserialize))]
#[br(big)]
pub struct Model {
    #[br(temp)] _magic: [u8; 2],
    #[br(temp)] _version: [u8; 2],

    pub skeleton_key: u32,
    pub joint_count: u32,
    pub elements_count: u32,
    pub elements_offset: u32,
    pub material_count: u32,
    pub material_offset: u32,
    pub bounds: [f32; 4],

    // Seek to 0xC, read the pointer, calculate base address
    #[br(seek_before = SeekFrom::Start(0xC), restore_position)]
    #[br(temp)]
    mesh_table_ptr: MeshTablePointer,

    // Now we read the elements using that calculated pointer
    #[br(
        seek_before = SeekFrom::Start(mesh_table_ptr.base_address), 
        count = elements_count
    )]
    pub elements: Vec<Element>,
}

/// Helper to parse the header pointer logic to find the mesh table.
#[derive(Debug)]
struct MeshTablePointer {
    base_address: u64,
}

impl BinRead for MeshTablePointer {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let _ = <u32>::read_options(reader, endian, ())?;
        let dpos = reader.stream_position()?;
        let pointer = <u32>::read_options(reader, endian, ())?;
        Ok(Self {
            base_address: dpos + pointer as u64 + 8,
        })
    }
}

/// Represents a mesh element within the model.
#[cfg_attr(feature = "export", derive(Serialize, Deserialize))]
#[binread]
#[br(big)]
pub struct Element {
    // 0x00
    pub num_faces: i32,

    // 0x04: Read the raw offset for export, then rewind so RelPtr can use it
    #[br(restore_position)]
    pub f_offset: i32,
    
    // The smart pointer that actually jumps and reads the data
    #[br(args { count: if num_faces > 0 { num_faces as usize * 2 } else { 0 } })]
    pub indices: RelPtr<Vec<u8>>,

    // 0x08
    pub num_vct: i32,
    // 0x0C
    pub vertex_stride: i32,

    // 0x10: Read raw offset for 'mat_index', then rewind
    #[br(restore_position)]
    pub v_offset: i32,

    // The smart pointer for vertices
    #[br(args { 
        count: if num_vct > 0 && vertex_stride > 0 { 
            (num_vct as usize) * (vertex_stride as usize) 
        } else { 0 } 
    })]
    pub vertices: RelPtr<Vec<u8>>,

    // 0x14: Originally '_reserved[0..4]' -> flags
    pub flags: u32,
    
    // 0x18: Originally '_reserved[4..8]' -> stream_ofs
    pub stream_ofs: u32,

    // 0x1C: Read raw offset for 'index_ofs', then rewind
    #[br(restore_position)]
    pub m_offset1: u32,

    // The smart triple-indirect material reader
    pub material_name: IndirectMaterial,

    // 0x20: Originally first 4 bytes of 'extra' -> vertex_ofs
    pub vertex_ofs: u32,

    // 0x24: The rest of the 0x4C struct (76 - 36 = 40 bytes)
    #[br(count = 40)]
    pub extra: Vec<u8>,
}
