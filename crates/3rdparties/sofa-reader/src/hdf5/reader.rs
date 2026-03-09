use crate::error::{Result, SofaError};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const HDF5_SIGNATURE: [u8; 8] = [0x89, b'H', b'D', b'F', 0x0d, 0x0a, 0x1a, 0x0a];
const UNDEF_ADDR: u64 = 0xFFFF_FFFF_FFFF_FFFF;

// Object header message types
const MSG_DATASPACE: u8 = 0x01;
const MSG_LINK_INFO: u8 = 0x02;
const MSG_DATATYPE: u8 = 0x03;
const MSG_FILL_VALUE_OLD: u8 = 0x04;
const MSG_FILL_VALUE: u8 = 0x05;
const MSG_LINK: u8 = 0x06;
const MSG_DATA_LAYOUT: u8 = 0x08;
const MSG_GROUP_INFO: u8 = 0x0A;
const MSG_FILTER_PIPELINE: u8 = 0x0B;
const MSG_ATTRIBUTE: u8 = 0x0C;
const MSG_OH_CONTINUATION: u8 = 0x10;
const MSG_SYMBOL_TABLE: u8 = 0x11;
const MSG_ATTR_INFO: u8 = 0x15;

#[derive(Debug, Clone)]
pub enum AttrValue {
    String(String),
    Float32(f32),
    Float64(f64),
    Int32(i32),
    Float32Array(Vec<f32>),
    Float64Array(Vec<f64>),
    Int32Array(Vec<i32>),
    Uint8Array(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DType {
    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Float32,
    Float64,
    FixedString(usize),
    VariableString,
}

impl DType {
    fn element_size(&self) -> usize {
        match self {
            DType::Int8 | DType::Uint8 => 1,
            DType::Int16 | DType::Uint16 => 2,
            DType::Int32 | DType::Uint32 | DType::Float32 => 4,
            DType::Int64 | DType::Uint64 | DType::Float64 => 8,
            DType::FixedString(n) => *n,
            DType::VariableString => 16, // HDF5 vlen type is a struct
        }
    }
}

impl std::fmt::Display for DType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
enum Layout {
    Compact {
        data: Vec<u8>,
    },
    Contiguous {
        address: u64,
        size: u64,
    },
    Chunked {
        address: u64,
        chunk_dims: Vec<u32>,
        filters: Vec<Filter>,
    },
}

#[derive(Debug, Clone)]
struct Filter {
    id: u16,
    _flags: u16,
    _client_data: Vec<u32>,
}

#[derive(Debug)]
struct DatasetInfo {
    dims: Vec<u64>,
    dtype: DType,
    layout: Layout,
}

#[derive(Debug)]
struct GroupInfo {
    children: HashMap<String, u64>,
    attributes: HashMap<String, AttrValue>,
}

pub struct Hdf5File {
    data: Vec<u8>,
    off_size: u8,
    len_size: u8,
    base_addr: u64,
    // Caches
    datasets: HashMap<String, DatasetInfo>,
    attributes: HashMap<String, AttrValue>,
    dimensions: HashMap<String, u64>,
}

// Low-level reading helpers
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    off_size: u8,
    len_size: u8,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], pos: usize, off_size: u8, len_size: u8) -> Self {
        Self {
            data,
            pos,
            off_size,
            len_size,
        }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn check(&self, need: usize) -> Result<()> {
        if self.pos + need > self.data.len() {
            return Err(SofaError::Truncated {
                offset: self.pos as u64,
                need: need as u64,
                have: self.remaining() as u64,
            });
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8> {
        self.check(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16> {
        self.check(2)?;
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32> {
        self.check(4)?;
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn u64(&mut self) -> Result<u64> {
        self.check(8)?;
        let v = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    fn offset(&mut self) -> Result<u64> {
        self.read_sized(self.off_size)
    }

    fn length(&mut self) -> Result<u64> {
        self.read_sized(self.len_size)
    }

    fn read_sized(&mut self, size: u8) -> Result<u64> {
        match size {
            2 => self.u16().map(|v| v as u64),
            4 => self.u32().map(|v| v as u64),
            8 => self.u64(),
            _ => {
                self.check(size as usize)?;
                let mut v = 0u64;
                for i in 0..size as usize {
                    v |= (self.data[self.pos + i] as u64) << (i * 8);
                }
                self.pos += size as usize;
                Ok(v)
            }
        }
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.check(n)?;
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        self.check(n)?;
        self.pos += n;
        Ok(())
    }

    fn slice_at(&self, offset: usize) -> Result<&'a [u8]> {
        if offset >= self.data.len() {
            return Err(SofaError::Truncated {
                offset: offset as u64,
                need: 1,
                have: 0,
            });
        }
        Ok(&self.data[offset..])
    }
}

impl Hdf5File {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = fs::read(path)?;
        Self::from_bytes(data)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        if data.len() < 8 || data[..8] != HDF5_SIGNATURE {
            return Err(SofaError::NotHdf5);
        }

        let sb_version = data[8];
        let (off_size, len_size, base_addr, root_addr) = match sb_version {
            0 | 1 => Self::parse_superblock_v0(&data)?,
            2 | 3 => Self::parse_superblock_v2(&data)?,
            v => return Err(SofaError::UnsupportedSuperblock(v)),
        };

        let mut file = Self {
            data,
            off_size,
            len_size,
            base_addr,
            datasets: HashMap::new(),
            attributes: HashMap::new(),
            dimensions: HashMap::new(),
        };

        file.parse_root_group(root_addr)?;
        Ok(file)
    }

    fn parse_superblock_v0(data: &[u8]) -> Result<(u8, u8, u64, u64)> {
        let mut c = Cursor::new(data, 8, 8, 8);
        let _version = c.u8()?; // already checked
        let _free_space_version = c.u8()?;
        let _root_group_sym_version = c.u8()?;
        let _reserved = c.u8()?;
        let _shared_header_version = c.u8()?;
        let off_size = c.u8()?;
        let len_size = c.u8()?;
        let _reserved2 = c.u8()?;
        c.off_size = off_size;
        c.len_size = len_size;
        let _group_leaf_k = c.u16()?;
        let _group_internal_k = c.u16()?;
        let _consistency_flags = c.u32()?;

        if _version == 1 {
            let _indexed_k = c.u16()?;
            let _reserved3 = c.u16()?;
        }

        let base_addr = c.offset()?;
        let _free_space_addr = c.offset()?;
        let _eof_addr = c.offset()?;
        let _driver_info_addr = c.offset()?;

        // Root group symbol table entry
        let _link_name_offset = c.offset()?;
        let root_obj_addr = c.offset()?;
        let cache_type = c.u32()?;
        let _reserved4 = c.u32()?;

        if cache_type == 1 {
            // Group: scratch-pad has B-tree address and name heap address
            let _btree_addr = c.offset()?;
            let _name_heap_addr = c.offset()?;
        }

        Ok((off_size, len_size, base_addr, root_obj_addr))
    }

    fn parse_superblock_v2(data: &[u8]) -> Result<(u8, u8, u64, u64)> {
        let mut c = Cursor::new(data, 8, 8, 8);
        let _version = c.u8()?;
        let off_size = c.u8()?;
        let len_size = c.u8()?;
        let _flags = c.u8()?;
        c.off_size = off_size;
        c.len_size = len_size;
        let base_addr = c.offset()?;
        let _sb_ext_addr = c.offset()?;
        let _eof_addr = c.offset()?;
        let root_addr = c.offset()?;
        // checksum follows
        Ok((off_size, len_size, base_addr, root_addr))
    }

    fn abs_offset(&self, addr: u64) -> usize {
        (addr - self.base_addr) as usize
    }

    fn cursor_at(&self, addr: u64) -> Cursor<'_> {
        Cursor::new(
            &self.data,
            self.abs_offset(addr),
            self.off_size,
            self.len_size,
        )
    }

    fn parse_root_group(&mut self, root_addr: u64) -> Result<()> {
        log::debug!("Parsing root group at addr 0x{:x}", root_addr);
        let group = self.parse_group(root_addr)?;
        log::debug!("Root group has {} children, {} attributes", group.children.len(), group.attributes.len());

        // Root group attributes are the global attributes
        self.attributes = group.attributes;

        // Process children: datasets become variables, dimension scales become dimensions
        for (name, obj_addr) in &group.children {
            match self.parse_dataset(*obj_addr) {
            Ok(ds) => {
                // Check if this is a dimension scale (NetCDF dimension)
                // Dimension scales have a CLASS=DIMENSION_SCALE attribute
                // But the dimension size is just the first extent of the dataspace
                if !ds.dims.is_empty() {
                    // For datasets with a single dimension and the same name as common dims
                    // they're likely dimension references
                    // We'll also store them as datasets for variable access
                }
                self.datasets.insert(name.clone(), ds);
            }
            Err(e) => {
            }
            }
        }

        // Now detect dimensions from dimension scale datasets
        // In NetCDF4, dimensions are stored as datasets with CLASS=DIMENSION_SCALE attribute
        // But we can also just look at the dataset shapes and known dimension names
        self.detect_dimensions()?;

        Ok(())
    }

    fn detect_dimensions(&mut self) -> Result<()> {
        // In SOFA files, dimension names are M, R, N, C, E, I
        // They are stored as 1-D datasets that serve as dimension scales
        let dim_candidates: Vec<(String, u64)> = self
            .datasets
            .iter()
            .filter_map(|(name, ds)| {
                if ds.dims.len() == 1 {
                    Some((name.clone(), ds.dims[0]))
                } else {
                    None
                }
            })
            .collect();

        for (name, size) in dim_candidates {
            self.dimensions.insert(name, size);
        }

        // Also check if any multi-dim datasets reference known dims by shape
        // For example, Data.IR has shape [M, R, N] - we can infer dimensions
        // from the variable shapes if needed

        Ok(())
    }

    fn parse_group(&self, addr: u64) -> Result<GroupInfo> {
        let offset = self.abs_offset(addr);
        let mut group = GroupInfo {
            children: HashMap::new(),
            attributes: HashMap::new(),
        };

        // Check if this is v1 or v2 object header
        if offset + 4 <= self.data.len() && &self.data[offset..offset + 4] == b"OHDR" {
            self.parse_oh_v2(addr, &mut group)?;
        } else {
            self.parse_oh_v1(addr, &mut group)?;
        }

        Ok(group)
    }

    fn parse_oh_v1(&self, addr: u64, group: &mut GroupInfo) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let version = c.u8()?;
        if version != 1 {
            return Err(SofaError::UnsupportedObjectHeader(version));
        }
        let _reserved = c.u8()?;
        let num_messages = c.u16()?;
        let _ref_count = c.u32()?;
        let header_size = c.u32()?;
        // padding to 8 bytes
        if header_size > 0 {
            // messages follow, aligned
        }

        let msg_start = c.pos;
        let msg_end = msg_start + header_size as usize;

        let mut i = 0u16;
        while i < num_messages && c.pos < msg_end {
            let msg_type = c.u16()? as u8;
            let msg_size = c.u16()? as usize;
            let _msg_flags = c.u8()?;
            c.skip(3)?; // reserved

            let msg_data_start = c.pos;

            match msg_type {
                MSG_SYMBOL_TABLE => {
                    self.parse_symbol_table_msg(&mut c, group)?;
                }
                MSG_ATTRIBUTE => {
                    self.parse_attribute_msg_v1(
                        &self.data[msg_data_start..msg_data_start + msg_size],
                        &mut group.attributes,
                    )?;
                }
                MSG_OH_CONTINUATION => {
                    let cont_addr = c.offset()?;
                    let cont_len = c.length()?;
                    if cont_addr != UNDEF_ADDR && cont_len > 0 {
                        self.parse_oh_v1_continuation(cont_addr, cont_len, group)?;
                    }
                    c.pos = msg_data_start + msg_size;
                    i += 1;
                    continue;
                }
                _ => {}
            }

            c.pos = msg_data_start + msg_size;
            i += 1;
        }
        Ok(())
    }

    fn parse_oh_v1_continuation(
        &self,
        addr: u64,
        _len: u64,
        group: &mut GroupInfo,
    ) -> Result<()> {
        // Continuation block has raw messages (no header)
        let mut c = self.cursor_at(addr);
        let end = c.pos + _len as usize;

        while c.pos + 8 <= end {
            let msg_type = c.u16()? as u8;
            let msg_size = c.u16()? as usize;
            let _msg_flags = c.u8()?;
            c.skip(3)?;
            let msg_data_start = c.pos;

            match msg_type {
                MSG_ATTRIBUTE => {
                    self.parse_attribute_msg_v1(
                        &self.data[msg_data_start..msg_data_start + msg_size],
                        &mut group.attributes,
                    )?;
                }
                MSG_SYMBOL_TABLE => {
                    self.parse_symbol_table_msg(&mut c, group)?;
                }
                MSG_OH_CONTINUATION => {
                    let cont_addr = c.offset()?;
                    let cont_len = c.length()?;
                    if cont_addr != UNDEF_ADDR && cont_len > 0 {
                        self.parse_oh_v1_continuation(cont_addr, cont_len, group)?;
                    }
                }
                _ => {}
            }

            c.pos = msg_data_start + msg_size;
        }
        Ok(())
    }

    fn parse_symbol_table_msg(&self, c: &mut Cursor<'_>, group: &mut GroupInfo) -> Result<()> {
        let btree_addr = c.offset()?;
        let heap_addr = c.offset()?;

        // Parse local heap for name lookup
        let heap_data = self.parse_local_heap(heap_addr)?;

        // Traverse B-tree v1 to find all symbol table nodes
        self.parse_btree_v1(btree_addr, &heap_data, group)?;

        Ok(())
    }

    fn parse_local_heap(&self, addr: u64) -> Result<Vec<u8>> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"HEAP" {
            return Err(SofaError::InvalidStructure("Bad local heap signature".into()));
        }
        let _version = c.u8()?;
        c.skip(3)?; // reserved
        let data_size = c.length()?;
        let _free_list_offset = c.length()?;
        let data_addr = c.offset()?;

        let off = self.abs_offset(data_addr);
        if off + data_size as usize > self.data.len() {
            return Err(SofaError::Truncated {
                offset: data_addr,
                need: data_size,
                have: (self.data.len() - off) as u64,
            });
        }
        Ok(self.data[off..off + data_size as usize].to_vec())
    }

    fn parse_btree_v1(
        &self,
        addr: u64,
        heap_data: &[u8],
        group: &mut GroupInfo,
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"TREE" {
            return Err(SofaError::InvalidStructure("Bad B-tree v1 signature".into()));
        }
        let node_type = c.u8()?;
        let node_level = c.u8()?;
        let entries_used = c.u16()?;
        let _left_sibling = c.offset()?;
        let _right_sibling = c.offset()?;

        if node_type != 0 {
            return Err(SofaError::InvalidStructure(format!(
                "Expected group B-tree (type 0), got type {}",
                node_type
            )));
        }

        if node_level == 0 {
            // Leaf node: entries point to symbol table nodes (SNODs)
            for _ in 0..entries_used {
                let _key = c.length()?;
                let child_addr = c.offset()?;
                self.parse_symbol_table_node(child_addr, heap_data, group)?;
            }
        } else {
            // Internal node: entries point to child B-tree nodes
            for _ in 0..entries_used {
                let _key = c.length()?;
                let child_addr = c.offset()?;
                self.parse_btree_v1(child_addr, heap_data, group)?;
            }
            // One more child pointer after last key
            // Actually for B-tree v1, there are entries_used keys and entries_used+1 children
            // But the format stores key,child pairs. Need to handle the extra child.
            // For group B-trees, the children are SNODs, and entries_used gives the count.
        }

        Ok(())
    }

    fn parse_symbol_table_node(
        &self,
        addr: u64,
        heap_data: &[u8],
        group: &mut GroupInfo,
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"SNOD" {
            return Err(SofaError::InvalidStructure("Bad SNOD signature".into()));
        }
        let _version = c.u8()?;
        let _reserved = c.u8()?;
        let num_symbols = c.u16()?;

        for _ in 0..num_symbols {
            let name_offset = c.offset()?;
            let obj_header_addr = c.offset()?;
            let _cache_type = c.u32()?;
            let _reserved2 = c.u32()?;
            c.skip(16)?; // scratch-pad space

            // Read name from local heap
            let name = self.read_heap_string(heap_data, name_offset as usize);
            if !name.is_empty() {
                group.children.insert(name, obj_header_addr);
            }
        }

        Ok(())
    }

    fn read_heap_string(&self, heap_data: &[u8], offset: usize) -> String {
        if offset >= heap_data.len() {
            return String::new();
        }
        let end = heap_data[offset..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(heap_data.len() - offset);
        String::from_utf8_lossy(&heap_data[offset..offset + end]).into_owned()
    }

    // ---- Object Header v2 ----

    fn parse_oh_v2(&self, addr: u64, group: &mut GroupInfo) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"OHDR" {
            return Err(SofaError::InvalidStructure("Bad OHDR signature".into()));
        }
        let version = c.u8()?;
        if version != 2 {
            return Err(SofaError::UnsupportedObjectHeader(version));
        }
        let flags = c.u8()?;

        // Optional timestamps
        if flags & 0x20 != 0 {
            c.skip(16)?; // 4 × 4-byte timestamps
        }

        // Optional phase change values
        if flags & 0x10 != 0 {
            let _max_compact = c.u16()?;
            let _min_dense = c.u16()?;
        }

        // Chunk #0 size
        let chunk_size_bytes = 1usize << (flags & 0x03);
        let chunk0_size = c.read_sized(chunk_size_bytes as u8)? as usize;
        let creation_order_tracked = flags & 0x04 != 0;

        let chunk_data_start = c.pos;
        let chunk_data_end = chunk_data_start + chunk0_size;

        self.parse_oh_v2_messages(
            &mut c,
            chunk_data_end,
            creation_order_tracked,
            group,
        )?;

        Ok(())
    }

    fn parse_oh_v2_messages(
        &self,
        c: &mut Cursor<'_>,
        end: usize,
        creation_order_tracked: bool,
        group: &mut GroupInfo,
    ) -> Result<()> {
        while c.pos + 4 <= end {
            let msg_type = c.u8()?;
            let msg_size = c.u16()? as usize;
            let _msg_flags = c.u8()?;
            if creation_order_tracked {
                let _creation_order = c.u16()?;
            }

            if msg_type == 0 && msg_size == 0 {
                break; // NIL message, end of messages
            }

            let msg_data_start = c.pos;

            match msg_type {
                MSG_LINK_INFO => {
                    self.parse_link_info_msg(
                        &self.data[msg_data_start..msg_data_start + msg_size],
                        group,
                    )?;
                }
                MSG_LINK => {
                    self.parse_link_msg(
                        &self.data[msg_data_start..msg_data_start + msg_size],
                        group,
                    )?;
                }
                MSG_ATTRIBUTE => {
                    self.parse_attribute_msg_v2(
                        &self.data[msg_data_start..msg_data_start + msg_size],
                        &mut group.attributes,
                    )?;
                }
                MSG_ATTR_INFO => {
                    self.parse_attr_info_msg(
                        &self.data[msg_data_start..msg_data_start + msg_size],
                        &mut group.attributes,
                    )?;
                }
                MSG_SYMBOL_TABLE => {
                    let mut mc = Cursor::new(self.data.as_slice(), msg_data_start, self.off_size, self.len_size);
                    self.parse_symbol_table_msg(&mut mc, group)?;
                }
                MSG_OH_CONTINUATION => {
                    let mut mc = Cursor::new(self.data.as_slice(), msg_data_start, self.off_size, self.len_size);
                    let cont_addr = mc.offset()?;
                    let cont_len = mc.length()?;
                    if cont_addr != UNDEF_ADDR && cont_len > 0 {
                        self.parse_oh_v2_continuation(cont_addr, cont_len, creation_order_tracked, group)?;
                    }
                }
                _ => {
                    // Skip unknown messages
                }
            }

            c.pos = msg_data_start + msg_size;
        }
        Ok(())
    }

    fn parse_oh_v2_continuation(
        &self,
        addr: u64,
        len: u64,
        creation_order_tracked: bool,
        group: &mut GroupInfo,
    ) -> Result<()> {
        let offset = self.abs_offset(addr);
        // v2 continuation blocks start with "OCHK" signature
        if offset + 4 <= self.data.len() && &self.data[offset..offset + 4] == b"OCHK" {
            let mut c = Cursor::new(&self.data, offset + 4, self.off_size, self.len_size);
            // End is addr + len - 4 (for checksum)
            let end = offset + len as usize - 4;
            self.parse_oh_v2_messages(&mut c, end, creation_order_tracked, group)?;
        } else {
            // Might be raw messages without OCHK signature
            let mut c = self.cursor_at(addr);
            let end = self.abs_offset(addr) + len as usize;
            self.parse_oh_v2_messages(&mut c, end, creation_order_tracked, group)?;
        }
        Ok(())
    }

    // ---- Link Info Message (dense link storage) ----

    fn parse_link_info_msg(&self, data: &[u8], group: &mut GroupInfo) -> Result<()> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let _version = c.u8()?;
        let flags = c.u8()?;

        if flags & 0x01 != 0 {
            let _max_creation_order = c.u64()?;
        }

        let fh_addr = c.offset()?;
        let name_btree_addr = c.offset()?;

        if flags & 0x01 != 0 {
            let _creation_order_btree_addr = c.offset()?;
        }

        if fh_addr == UNDEF_ADDR || name_btree_addr == UNDEF_ADDR {
            return Ok(()); // Empty group
        }

        // Parse fractal heap + B-tree v2 for links
        let fh = self.parse_fractal_heap_header(fh_addr)?;
        let records = self.parse_btree_v2(name_btree_addr, &fh)?;

        for record in &records {
            if let Some((name, addr)) = self.read_link_from_heap(&fh, record)? {
                group.children.insert(name, addr);
            }
        }

        Ok(())
    }

    // ---- Attribute Info Message (dense attribute storage) ----

    fn parse_attr_info_msg(
        &self,
        data: &[u8],
        attrs: &mut HashMap<String, AttrValue>,
    ) -> Result<()> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let _version = c.u8()?;
        let flags = c.u8()?;

        if flags & 0x01 != 0 {
            let _max_creation_order = c.u16()?;
        }

        let fh_addr = c.offset()?;
        let name_btree_addr = c.offset()?;

        if flags & 0x01 != 0 {
            let _creation_order_btree = c.offset()?;
        }

        if fh_addr == UNDEF_ADDR || name_btree_addr == UNDEF_ADDR {
            return Ok(());
        }

        let fh = self.parse_fractal_heap_header(fh_addr)?;
        let records = self.parse_btree_v2(name_btree_addr, &fh)?;

        for record in &records {
            if let Some(attr_data) = self.read_managed_object(&fh, record)? {
                // Parse attribute from the heap data
                self.parse_attribute_from_bytes(&attr_data, attrs)?;
            }
        }

        Ok(())
    }

    // ---- Link Message (compact storage) ----

    fn parse_link_msg(&self, data: &[u8], group: &mut GroupInfo) -> Result<()> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let _version = c.u8()?;
        let flags = c.u8()?;

        // Link type
        let link_type = if flags & 0x08 != 0 {
            c.u8()?
        } else {
            0 // hard link
        };

        if flags & 0x04 != 0 {
            let _creation_order = c.u64()?;
        }

        // Link name character set
        if flags & 0x10 != 0 {
            let _charset = c.u8()?;
        }

        // Name size
        let name_size_bytes = 1usize << (flags & 0x03);
        let name_len = c.read_sized(name_size_bytes as u8)? as usize;
        let name_bytes = c.bytes(name_len)?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();

        if link_type == 0 {
            // Hard link
            let target_addr = c.offset()?;
            group.children.insert(name, target_addr);
        }
        // Soft/external links not needed for SOFA

        Ok(())
    }

}

// Pull FractalHeapInfo out of impl block
#[derive(Debug)]
struct FractalHeapInfo {
    heap_id_length: u16,
    max_heap_size: u16,
    starting_block_size: u64,
    max_direct_block_size: u64,
    table_width: u16,
    root_block_address: u64,
    current_rows: u16,
    num_managed_objects: u64,
    address: u64,
    io_filter_encoded_length: u16,
}

#[derive(Debug)]
struct HeapRecord {
    heap_id: Vec<u8>,
}

impl Hdf5File {
    fn parse_fractal_heap_header(&self, addr: u64) -> Result<FractalHeapInfo> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"FRHP" {
            return Err(SofaError::InvalidStructure("Bad FRHP signature".into()));
        }
        let _version = c.u8()?;
        let heap_id_length = c.u16()?;
        let io_filter_encoded_length = c.u16()?;
        let _flags = c.u8()?;
        let _max_managed_obj_size = c.u32()?;
        let _next_huge_id = c.length()?;
        let _huge_btree_addr = c.offset()?;
        let _free_space_managed = c.length()?;
        let _free_space_mgr_addr = c.offset()?;
        let _managed_space = c.length()?;
        let _allocated_managed_space = c.length()?;
        let _iter_offset = c.length()?;
        let num_managed_objects = c.length()?;
        let _size_huge = c.length()?;
        let _num_huge = c.length()?;
        let _size_tiny = c.length()?;
        let _num_tiny = c.length()?;
        let table_width = c.u16()?;
        let starting_block_size = c.length()?;
        let max_direct_block_size = c.length()?;
        let max_heap_size = c.u16()?;
        let _starting_rows = c.u16()?;
        let root_block_address = c.offset()?;
        let current_rows = c.u16()?;

        if io_filter_encoded_length > 0 {
            let _filter_size = c.length()?;
            let _filter_mask = c.u32()?;
        }
        // checksum follows

        Ok(FractalHeapInfo {
            heap_id_length,
            max_heap_size,
            starting_block_size,
            max_direct_block_size,
            table_width,
            root_block_address,
            current_rows,
            num_managed_objects,
            address: addr,
            io_filter_encoded_length,
        })
    }

    fn parse_btree_v2(
        &self,
        addr: u64,
        fh: &FractalHeapInfo,
    ) -> Result<Vec<HeapRecord>> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"BTHD" {
            return Err(SofaError::InvalidStructure("Bad BTHD signature".into()));
        }
        let _version = c.u8()?;
        let btype = c.u8()?;
        let _node_size = c.u32()?;
        let record_size = c.u16()?;
        let depth = c.u16()?;
        let _split_percent = c.u8()?;
        let _merge_percent = c.u8()?;
        let root_addr = c.offset()?;
        let num_records_root = c.u16()?;
        let _total_records = c.length()?;
        // checksum

        if root_addr == UNDEF_ADDR || num_records_root == 0 {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        self.parse_btree_v2_node(
            root_addr,
            depth,
            num_records_root,
            record_size,
            btype,
            fh,
            &mut records,
        )?;

        Ok(records)
    }

    fn parse_btree_v2_node(
        &self,
        addr: u64,
        depth: u16,
        num_records: u16,
        record_size: u16,
        btype: u8,
        fh: &FractalHeapInfo,
        records: &mut Vec<HeapRecord>,
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);

        if depth == 0 {
            // Leaf node: BTLF
            let sig = c.bytes(4)?;
            if sig != b"BTLF" {
                return Err(SofaError::InvalidStructure("Bad BTLF signature".into()));
            }
            let _version = c.u8()?;
            let _type = c.u8()?;

            for _ in 0..num_records {
                let rec_start = c.pos;
                // Record format depends on btype:
                // Type 5 (group dense name): hash(4) + heap_id(heap_id_length)
                // Type 6 (group dense corder): creation_order(8) + heap_id(heap_id_length)
                // Type 8 (attr dense name): creation_order(8) + hash(4) + heap_id(heap_id_length)
                // Type 9 (attr dense corder): attr info...
                let heap_id = match btype {
                    5 => {
                        // Group links by name hash
                        c.skip(4)?; // hash
                        c.bytes(fh.heap_id_length as usize)?.to_vec()
                    }
                    6 => {
                        // Group links by creation order
                        c.skip(8)?; // creation_order
                        c.bytes(fh.heap_id_length as usize)?.to_vec()
                    }
                    8 => {
                        // Attributes by name: heap_id(heap_id_length) + flags(1) + creation_order(4) + hash(4)
                        c.bytes(fh.heap_id_length as usize)?.to_vec()
                    }
                    9 => {
                        // Attributes by creation order: heap_id(heap_id_length) + flags(1)
                        c.bytes(fh.heap_id_length as usize)?.to_vec()
                    }
                    _ => {
                        // Generic: skip to heap_id at end
                        let extra = record_size as usize - fh.heap_id_length as usize;
                        c.skip(extra)?;
                        c.bytes(fh.heap_id_length as usize)?.to_vec()
                    }
                };

                // Ensure we consumed exactly record_size bytes
                c.pos = rec_start + record_size as usize;

                records.push(HeapRecord { heap_id });
            }
        } else {
            // Internal node: BTIN
            let sig = c.bytes(4)?;
            if sig != b"BTIN" {
                return Err(SofaError::InvalidStructure("Bad BTIN signature".into()));
            }
            let _version = c.u8()?;
            let _type = c.u8()?;

            // Records then child node pointers
            // First read all records
            let rec_start_pos = c.pos;
            for _ in 0..num_records {
                let rec_start = c.pos;
                let extra = record_size as usize - fh.heap_id_length as usize;
                c.skip(extra)?;
                let heap_id = c.bytes(fh.heap_id_length as usize)?.to_vec();
                c.pos = rec_start + record_size as usize;
                records.push(HeapRecord { heap_id });
            }

            // Child node pointers: (num_records + 1) entries
            // Each: address(off_size) + num_records(variable) + total_records(variable)
            // The num_records and total_records sizes depend on depth and max records
            // For simplicity, we'll recursively process by finding child nodes
            for _ in 0..=num_records {
                let child_addr = c.offset()?;
                // num_records_in_child size depends on the max records possible at depth-1
                // For simplicity, read as 2 bytes (which handles up to 65535 records)
                let child_num_records = c.u16()? as u16;
                if depth > 1 {
                    let _total_in_subtree = c.length()?;
                }
                if child_addr != UNDEF_ADDR {
                    self.parse_btree_v2_node(
                        child_addr,
                        depth - 1,
                        child_num_records,
                        record_size,
                        btype,
                        fh,
                        records,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn read_managed_object<'a>(
        &self,
        fh: &FractalHeapInfo,
        record: &HeapRecord,
    ) -> Result<Option<Vec<u8>>> {
        if record.heap_id.is_empty() {
            return Ok(None);
        }

        let id_byte0 = record.heap_id[0];
        let id_type = (id_byte0 >> 4) & 0x03; // bits 4-5

        match id_type {
            0 => {
                // Managed object
                self.read_managed_from_heap(fh, &record.heap_id)
            }
            1 => {
                // Tiny object: data is embedded in the heap ID
                let len = (id_byte0 & 0x0F) as usize + 1;
                if len + 1 <= record.heap_id.len() {
                    Ok(Some(record.heap_id[1..1 + len].to_vec()))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None), // Huge objects not expected for SOFA
        }
    }

    fn read_managed_from_heap(
        &self,
        fh: &FractalHeapInfo,
        heap_id: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        // Decode managed heap ID: byte0 is type/version, rest is offset+length
        let offset_bytes = (fh.max_heap_size as usize + 7) / 8;
        let length_bytes = fh.heap_id_length as usize - 1 - offset_bytes;

        if heap_id.len() < 1 + offset_bytes + length_bytes {
            return Ok(None);
        }

        let mut heap_offset = 0u64;
        for i in 0..offset_bytes {
            heap_offset |= (heap_id[1 + i] as u64) << (i * 8);
        }

        let mut obj_length = 0u64;
        for i in 0..length_bytes {
            obj_length |= (heap_id[1 + offset_bytes + i] as u64) << (i * 8);
        }

        if obj_length == 0 {
            return Ok(None);
        }

        // Find which direct block this offset falls into
        let block_data = self.find_direct_block_data(fh, heap_offset)?;

        if let Some((block_bytes, block_start_offset)) = block_data {
            let local_offset = (heap_offset - block_start_offset) as usize;
            if local_offset + obj_length as usize <= block_bytes.len() {
                return Ok(Some(
                    block_bytes[local_offset..local_offset + obj_length as usize].to_vec(),
                ));
            }
        }

        Ok(None)
    }

    fn find_direct_block_data(
        &self,
        fh: &FractalHeapInfo,
        heap_offset: u64,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        if fh.root_block_address == UNDEF_ADDR {
            return Ok(None);
        }

        if fh.current_rows == 0 {
            // Root is a direct block
            let data = self.read_direct_block(fh, fh.root_block_address, fh.starting_block_size)?;
            return Ok(Some((data, 0)));
        }

        // Root is an indirect block - traverse to find the right direct block
        self.find_in_indirect_block(
            fh,
            fh.root_block_address,
            fh.current_rows as usize,
            heap_offset,
            0,
        )
    }

    fn find_in_indirect_block(
        &self,
        fh: &FractalHeapInfo,
        addr: u64,
        nrows: usize,
        target_offset: u64,
        base_offset: u64,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"FHIB" {
            return Err(SofaError::InvalidStructure("Bad FHIB signature".into()));
        }
        let _version = c.u8()?;
        let _heap_header_addr = c.offset()?;

        // Block offset (ceil(max_heap_size / 8) bytes)
        let block_offset_size = ((fh.max_heap_size as usize) + 7) / 8;
        c.skip(block_offset_size)?;

        // Compute block sizes for each row
        let table_width = fh.table_width as u64;
        let mut current_offset = base_offset;

        // Direct block entries (first max_dblock_rows rows × table_width)
        let max_dblock_rows = Self::max_direct_block_rows(fh);
        let num_direct_rows = nrows.min(max_dblock_rows);
        let num_direct_entries = num_direct_rows * fh.table_width as usize;

        for row in 0..num_direct_rows {
            let block_size = Self::row_block_size(fh, row);
            for _ in 0..table_width {
                let child_addr = c.offset()?;
                if fh.io_filter_encoded_length > 0 {
                    let _filtered_size = c.length()?;
                    let _filter_mask = c.u32()?;
                }

                if child_addr != UNDEF_ADDR
                    && target_offset >= current_offset
                    && target_offset < current_offset + block_size
                {
                    let data = self.read_direct_block(fh, child_addr, block_size)?;
                    return Ok(Some((data, current_offset)));
                }
                current_offset += block_size;
            }
        }

        // Indirect block entries (remaining rows)
        for row in num_direct_rows..nrows {
            let block_size = Self::row_block_size(fh, row);
            let child_nrows = row - max_dblock_rows + 1;
            let subtree_size = Self::indirect_block_size(fh, child_nrows);

            for _ in 0..table_width {
                let child_addr = c.offset()?;
                if child_addr != UNDEF_ADDR
                    && target_offset >= current_offset
                    && target_offset < current_offset + subtree_size
                {
                    return self.find_in_indirect_block(
                        fh,
                        child_addr,
                        child_nrows,
                        target_offset,
                        current_offset,
                    );
                }
                current_offset += subtree_size;
            }
        }

        Ok(None)
    }

    fn max_direct_block_rows(fh: &FractalHeapInfo) -> usize {
        if fh.max_direct_block_size == 0 || fh.starting_block_size == 0 {
            return 0;
        }
        let log2_max = (fh.max_direct_block_size / fh.starting_block_size)
            .next_power_of_two()
            .trailing_zeros();
        // Row 0 and 1 have starting_block_size, row 2 has 2×starting, etc.
        (log2_max as usize) + 1
    }

    fn row_block_size(fh: &FractalHeapInfo, row: usize) -> u64 {
        if row < 2 {
            fh.starting_block_size
        } else {
            fh.starting_block_size * (1u64 << (row - 1))
        }
    }

    fn indirect_block_size(fh: &FractalHeapInfo, nrows: usize) -> u64 {
        let tw = fh.table_width as u64;
        let mut size = 0u64;
        for row in 0..nrows {
            size += Self::row_block_size(fh, row) * tw;
        }
        size
    }

    fn read_direct_block(
        &self,
        _fh: &FractalHeapInfo,
        addr: u64,
        block_size: u64,
    ) -> Result<Vec<u8>> {
        // Return the entire direct block as-is. Heap offsets are relative to
        // the start of the block (including the FHDB header).
        let off = self.abs_offset(addr);
        let end = off + block_size as usize;
        if end > self.data.len() {
            return Err(SofaError::Truncated {
                offset: addr,
                need: block_size,
                have: (self.data.len() - off) as u64,
            });
        }
        Ok(self.data[off..end].to_vec())
    }

    fn read_link_from_heap(
        &self,
        fh: &FractalHeapInfo,
        record: &HeapRecord,
    ) -> Result<Option<(String, u64)>> {
        let obj_data = self.read_managed_object(fh, record)?;
        if let Some(data) = obj_data {
            self.parse_link_from_bytes(&data)
        } else {
            Ok(None)
        }
    }

    fn parse_link_from_bytes(&self, data: &[u8]) -> Result<Option<(String, u64)>> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let _version = c.u8()?;
        let flags = c.u8()?;

        let link_type = if flags & 0x08 != 0 { c.u8()? } else { 0 };
        if flags & 0x04 != 0 {
            let _creation_order = c.u64()?;
        }
        if flags & 0x10 != 0 {
            let _charset = c.u8()?;
        }

        let name_size_bytes = 1usize << (flags & 0x03);
        let name_len = c.read_sized(name_size_bytes as u8)? as usize;
        let name_bytes = c.bytes(name_len)?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();

        if link_type == 0 {
            let target_addr = c.offset()?;
            Ok(Some((name, target_addr)))
        } else {
            Ok(None)
        }
    }

    // ---- Attribute parsing ----

    fn parse_attribute_msg_v1(
        &self,
        data: &[u8],
        attrs: &mut HashMap<String, AttrValue>,
    ) -> Result<()> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let version = c.u8()?;
        let _reserved = c.u8()?;
        let name_size = c.u16()? as usize;
        let dt_size = c.u16()? as usize;
        let ds_size = c.u16()? as usize;

        // Name (null-terminated, padded to 8-byte boundary)
        let name_bytes = c.bytes(name_size)?;
        let name = self.null_terminated_string(name_bytes);

        if version < 3 {
            // Pad name to 8-byte boundary
            let padded_name = (name_size + 7) & !7;
            c.pos = c.pos - name_size + padded_name;
        }

        // Datatype
        let dt_start = c.pos;
        let dtype = self.parse_datatype_msg(&data[dt_start..])?;
        if version < 3 {
            c.pos = dt_start + ((dt_size + 7) & !7);
        } else {
            c.pos = dt_start + dt_size;
        }

        // Dataspace
        let ds_start = c.pos;
        let dims = self.parse_dataspace_msg(&data[ds_start..])?;
        if version < 3 {
            c.pos = ds_start + ((ds_size + 7) & !7);
        } else {
            c.pos = ds_start + ds_size;
        }

        // Data
        let attr_data = &data[c.pos..];
        let value = self.interpret_attr_value(&dtype, &dims, attr_data);
        if let Some(v) = value {
            attrs.insert(name, v);
        }

        Ok(())
    }

    fn parse_attribute_msg_v2(
        &self,
        data: &[u8],
        attrs: &mut HashMap<String, AttrValue>,
    ) -> Result<()> {
        // v2 attribute messages have v3 format (no padding)
        self.parse_attribute_from_bytes(data, attrs)
    }

    fn parse_attribute_from_bytes(
        &self,
        data: &[u8],
        attrs: &mut HashMap<String, AttrValue>,
    ) -> Result<()> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let version = c.u8()?;

        if version >= 3 {
            let _flags = c.u8()?;
        } else {
            let _reserved = c.u8()?;
        }

        let name_size = c.u16()? as usize;
        let dt_size = c.u16()? as usize;
        let ds_size = c.u16()? as usize;

        if version >= 3 {
            let _encoding = c.u8()?;
        }

        // Name
        let name_bytes = c.bytes(name_size)?;
        let name = self.null_terminated_string(name_bytes);

        if version < 3 {
            let padded = (c.pos - (c.pos - name_size) + 7) & !7;
            c.pos = c.pos - name_size + padded.max(name_size);
        }

        // Datatype
        let dt_start = c.pos;
        let dtype = self.parse_datatype_msg(&data[dt_start..])?;
        c.pos = dt_start + dt_size;

        // Dataspace
        let ds_start = c.pos;
        let dims = self.parse_dataspace_msg(&data[ds_start..])?;
        c.pos = ds_start + ds_size;

        // Data
        let attr_data = &data[c.pos..];
        let value = self.interpret_attr_value(&dtype, &dims, attr_data);
        if let Some(v) = value {
            attrs.insert(name, v);
        }

        Ok(())
    }

    fn null_terminated_string(&self, bytes: &[u8]) -> String {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    fn interpret_attr_value(&self, dtype: &DType, dims: &[u64], data: &[u8]) -> Option<AttrValue> {
        let total_elements: u64 = dims.iter().product::<u64>().max(1);
        let elem_size = dtype.element_size();
        let needed = total_elements as usize * elem_size;
        if data.len() < needed {
            return None;
        }

        match dtype {
            DType::FixedString(n) => {
                if total_elements == 1 {
                    let end = data[..*n]
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(*n);
                    Some(AttrValue::String(
                        String::from_utf8_lossy(&data[..end]).into_owned(),
                    ))
                } else {
                    // Array of strings - concatenate
                    let s: Vec<String> = (0..total_elements as usize)
                        .map(|i| {
                            let start = i * n;
                            let chunk = &data[start..start + n];
                            let end = chunk.iter().position(|&b| b == 0).unwrap_or(*n);
                            String::from_utf8_lossy(&chunk[..end]).into_owned()
                        })
                        .collect();
                    Some(AttrValue::String(s.join("")))
                }
            }
            DType::VariableString => {
                // Variable-length string: stored as a pointer struct in HDF5
                // For global heap references: heap_addr(offset_size) + index(4)
                if data.len() >= self.off_size as usize + 4 {
                    let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
                    // The vlen data is: length(4) + global_heap_addr(off_size) + global_heap_index(4)
                    // Or it might be a direct pointer. Let me handle the global heap case.
                    let len = if let Ok(l) = c.u32() { l } else { return None };
                    let gh_addr = if let Ok(a) = c.offset() { a } else { return None };
                    let gh_index = if let Ok(i) = c.u32() { i } else { return None };

                    if gh_addr != UNDEF_ADDR && gh_addr != 0 {
                        if let Ok(s) = self.read_global_heap_string(gh_addr, gh_index, len) {
                            return Some(AttrValue::String(s));
                        }
                    }
                }
                None
            }
            DType::Float32 => {
                if total_elements == 1 && data.len() >= 4 {
                    Some(AttrValue::Float32(f32::from_le_bytes(
                        data[..4].try_into().ok()?,
                    )))
                } else {
                    let arr: Vec<f32> = data
                        .chunks_exact(4)
                        .take(total_elements as usize)
                        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    Some(AttrValue::Float32Array(arr))
                }
            }
            DType::Float64 => {
                if total_elements == 1 && data.len() >= 8 {
                    Some(AttrValue::Float64(f64::from_le_bytes(
                        data[..8].try_into().ok()?,
                    )))
                } else {
                    let arr: Vec<f64> = data
                        .chunks_exact(8)
                        .take(total_elements as usize)
                        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    Some(AttrValue::Float64Array(arr))
                }
            }
            DType::Int32 => {
                if total_elements == 1 && data.len() >= 4 {
                    Some(AttrValue::Int32(i32::from_le_bytes(
                        data[..4].try_into().ok()?,
                    )))
                } else {
                    let arr: Vec<i32> = data
                        .chunks_exact(4)
                        .take(total_elements as usize)
                        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    Some(AttrValue::Int32Array(arr))
                }
            }
            _ => {
                let n = (total_elements as usize) * elem_size;
                if n <= data.len() {
                    Some(AttrValue::Uint8Array(data[..n].to_vec()))
                } else {
                    None
                }
            }
        }
    }

    fn read_global_heap_string(
        &self,
        gh_addr: u64,
        index: u32,
        _expected_len: u32,
    ) -> Result<String> {
        let mut c = self.cursor_at(gh_addr);
        let sig = c.bytes(4)?;
        if sig != b"GCOL" {
            return Err(SofaError::InvalidStructure("Bad GCOL signature".into()));
        }
        let _version = c.u8()?;
        c.skip(3)?; // reserved
        let collection_size = c.length()?;

        let end = c.pos + collection_size as usize - 8 - self.len_size as usize;

        while c.pos < end {
            let obj_index = c.u16()?;
            if obj_index == 0 {
                break; // End of objects
            }
            let _ref_count = c.u16()?;
            c.skip(4)?; // reserved
            let obj_size = c.length()?;

            if obj_index as u32 == index {
                let str_data = c.bytes(obj_size as usize)?;
                let end = str_data.iter().position(|&b| b == 0).unwrap_or(str_data.len());
                return Ok(String::from_utf8_lossy(&str_data[..end]).into_owned());
            }

            // Skip object data + padding to 8-byte boundary
            let padded_size = (obj_size as usize + 7) & !7;
            c.pos = c.pos + padded_size;
        }

        Err(SofaError::InvalidStructure(format!(
            "Global heap object {} not found",
            index
        )))
    }

    // ---- Datatype parsing ----

    fn parse_datatype_msg(&self, data: &[u8]) -> Result<DType> {
        if data.len() < 8 {
            return Err(SofaError::Truncated {
                offset: 0,
                need: 8,
                have: data.len() as u64,
            });
        }

        let class_and_version = data[0];
        let class = class_and_version & 0x0F;
        let _version = (class_and_version >> 4) & 0x0F;
        let bit_field_0 = data[1];
        let bit_field_1 = data[2];
        let _bit_field_2 = data[3];
        let size = u32::from_le_bytes(data[4..8].try_into().unwrap());

        match class {
            0 => {
                // Fixed-point (integer)
                let signed = bit_field_0 & 0x08 != 0;
                match (size, signed) {
                    (1, false) => Ok(DType::Uint8),
                    (1, true) => Ok(DType::Int8),
                    (2, false) => Ok(DType::Uint16),
                    (2, true) => Ok(DType::Int16),
                    (4, false) => Ok(DType::Uint32),
                    (4, true) => Ok(DType::Int32),
                    (8, false) => Ok(DType::Uint64),
                    (8, true) => Ok(DType::Int64),
                    _ => Err(SofaError::Unsupported(format!(
                        "Integer type: size={}, signed={}",
                        size, signed
                    ))),
                }
            }
            1 => {
                // Floating-point
                match size {
                    4 => Ok(DType::Float32),
                    8 => Ok(DType::Float64),
                    _ => Err(SofaError::Unsupported(format!("Float size: {}", size))),
                }
            }
            3 => {
                // String
                let padding = bit_field_0 & 0x0F;
                // padding: 0=null-terminate, 1=null-pad, 2=space-pad
                Ok(DType::FixedString(size as usize))
            }
            9 => {
                // Variable-length
                // Check if it's a string type
                let vl_type = bit_field_0 & 0x0F;
                if vl_type == 1 {
                    // Variable-length string
                    Ok(DType::VariableString)
                } else {
                    // Variable-length sequence - treat as opaque
                    Ok(DType::VariableString) // approximation
                }
            }
            6 => {
                // Compound type - used by NetCDF4 dimension scales
                // We don't need to decode the members, just return a placeholder
                Ok(DType::Uint8) // placeholder - we only care about dataspace dims
            }
            _ => Err(SofaError::Unsupported(format!(
                "Datatype class: {}",
                class
            ))),
        }
    }

    // ---- Dataspace parsing ----

    fn parse_dataspace_msg(&self, data: &[u8]) -> Result<Vec<u64>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let version = c.u8()?;
        let rank = c.u8()? as usize;
        let flags = c.u8()?;

        if version == 1 {
            c.skip(5)?; // reserved
        } else if version == 2 {
            let _ds_type = c.u8()?; // 0=scalar, 1=simple, 2=null
        }

        let mut dims = Vec::with_capacity(rank);
        for _ in 0..rank {
            dims.push(c.length()?);
        }

        // Max dimensions (optional)
        if version == 1 || (flags & 0x01 != 0) {
            for _ in 0..rank {
                let _max = c.length()?;
            }
        }

        // Permutation indices (v1 only, if flags & 0x02)
        if version == 1 && (flags & 0x02 != 0) {
            for _ in 0..rank {
                let _perm = c.length()?;
            }
        }

        Ok(dims)
    }

    // ---- Dataset parsing ----

    fn parse_dataset(&self, addr: u64) -> Result<DatasetInfo> {
        let offset = self.abs_offset(addr);
        let mut ds = DatasetInfo {
            dims: Vec::new(),
            dtype: DType::Float32,
            layout: Layout::Contiguous {
                address: UNDEF_ADDR,
                size: 0,
            },
        };
        let mut attrs = HashMap::new();
        let mut filters = Vec::new();

        if offset + 4 <= self.data.len() && &self.data[offset..offset + 4] == b"OHDR" {
            self.parse_dataset_oh_v2(addr, &mut ds, &mut attrs, &mut filters)?;
        } else {
            self.parse_dataset_oh_v1(addr, &mut ds, &mut attrs, &mut filters)?;
        }

        // Apply filters to chunked layout
        if let Layout::Chunked {
            filters: ref mut layout_filters,
            ..
        } = ds.layout
        {
            *layout_filters = filters;
        }

        Ok(ds)
    }

    fn parse_dataset_oh_v1(
        &self,
        addr: u64,
        ds: &mut DatasetInfo,
        attrs: &mut HashMap<String, AttrValue>,
        filters: &mut Vec<Filter>,
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let version = c.u8()?;
        if version != 1 {
            return Err(SofaError::UnsupportedObjectHeader(version));
        }
        let _reserved = c.u8()?;
        let num_messages = c.u16()?;
        let _ref_count = c.u32()?;
        let header_size = c.u32()?;

        let msg_start = c.pos;
        let msg_end = msg_start + header_size as usize;

        let mut i = 0u16;
        while i < num_messages && c.pos < msg_end {
            let msg_type = c.u16()? as u8;
            let msg_size = c.u16()? as usize;
            let _msg_flags = c.u8()?;
            c.skip(3)?;

            let msg_data_start = c.pos;
            let msg_data = &self.data[msg_data_start..msg_data_start + msg_size];

            match msg_type {
                MSG_DATASPACE => {
                    ds.dims = self.parse_dataspace_msg(msg_data)?;
                }
                MSG_DATATYPE => {
                    ds.dtype = self.parse_datatype_msg(msg_data)?;
                }
                MSG_DATA_LAYOUT => {
                    ds.layout = self.parse_layout_msg(msg_data)?;
                }
                MSG_FILTER_PIPELINE => {
                    *filters = self.parse_filter_pipeline_msg(msg_data)?;
                }
                MSG_ATTRIBUTE => {
                    self.parse_attribute_msg_v1(msg_data, attrs)?;
                }
                MSG_OH_CONTINUATION => {
                    let mut mc = Cursor::new(self.data.as_slice(), msg_data_start, self.off_size, self.len_size);
                    let cont_addr = mc.offset()?;
                    let cont_len = mc.length()?;
                    if cont_addr != UNDEF_ADDR {
                        self.parse_dataset_oh_v1_continuation(
                            cont_addr, cont_len, ds, attrs, filters,
                        )?;
                    }
                }
                _ => {}
            }

            c.pos = msg_data_start + msg_size;
            i += 1;
        }
        Ok(())
    }

    fn parse_dataset_oh_v1_continuation(
        &self,
        addr: u64,
        len: u64,
        ds: &mut DatasetInfo,
        attrs: &mut HashMap<String, AttrValue>,
        filters: &mut Vec<Filter>,
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let end = self.abs_offset(addr) + len as usize;

        while c.pos + 8 <= end {
            let msg_type = c.u16()? as u8;
            let msg_size = c.u16()? as usize;
            let _msg_flags = c.u8()?;
            c.skip(3)?;

            let msg_data_start = c.pos;
            let msg_data = &self.data[msg_data_start..msg_data_start + msg_size];

            match msg_type {
                MSG_DATASPACE => ds.dims = self.parse_dataspace_msg(msg_data)?,
                MSG_DATATYPE => ds.dtype = self.parse_datatype_msg(msg_data)?,
                MSG_DATA_LAYOUT => ds.layout = self.parse_layout_msg(msg_data)?,
                MSG_FILTER_PIPELINE => *filters = self.parse_filter_pipeline_msg(msg_data)?,
                MSG_ATTRIBUTE => self.parse_attribute_msg_v1(msg_data, attrs)?,
                _ => {}
            }

            c.pos = msg_data_start + msg_size;
        }
        Ok(())
    }

    fn parse_dataset_oh_v2(
        &self,
        addr: u64,
        ds: &mut DatasetInfo,
        attrs: &mut HashMap<String, AttrValue>,
        filters: &mut Vec<Filter>,
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"OHDR" {
            return Err(SofaError::InvalidStructure("Bad OHDR signature".into()));
        }
        let _version = c.u8()?;
        let flags = c.u8()?;

        if flags & 0x20 != 0 {
            c.skip(16)?;
        }
        if flags & 0x10 != 0 {
            c.skip(4)?;
        }

        let chunk_size_bytes = 1usize << (flags & 0x03);
        let chunk0_size = c.read_sized(chunk_size_bytes as u8)? as usize;
        let creation_order_tracked = flags & 0x04 != 0;

        let chunk_data_start = c.pos;
        let chunk_data_end = chunk_data_start + chunk0_size;

        self.parse_dataset_oh_v2_messages(
            &mut c,
            chunk_data_end,
            creation_order_tracked,
            ds,
            attrs,
            filters,
        )?;

        Ok(())
    }

    fn parse_dataset_oh_v2_messages(
        &self,
        c: &mut Cursor<'_>,
        end: usize,
        creation_order_tracked: bool,
        ds: &mut DatasetInfo,
        attrs: &mut HashMap<String, AttrValue>,
        filters: &mut Vec<Filter>,
    ) -> Result<()> {
        while c.pos + 4 <= end {
            let msg_type = c.u8()?;
            let msg_size = c.u16()? as usize;
            let _msg_flags = c.u8()?;
            if creation_order_tracked {
                let _co = c.u16()?;
            }

            if msg_type == 0 && msg_size == 0 {
                break;
            }

            let msg_data_start = c.pos;
            if msg_data_start + msg_size > self.data.len() {
                break;
            }
            let msg_data = &self.data[msg_data_start..msg_data_start + msg_size];

            match msg_type {
                MSG_DATASPACE => ds.dims = self.parse_dataspace_msg(msg_data)?,
                MSG_DATATYPE => ds.dtype = self.parse_datatype_msg(msg_data)?,
                MSG_DATA_LAYOUT => ds.layout = self.parse_layout_msg(msg_data)?,
                MSG_FILTER_PIPELINE => *filters = self.parse_filter_pipeline_msg(msg_data)?,
                MSG_ATTRIBUTE => self.parse_attribute_msg_v2(msg_data, attrs)?,
                MSG_ATTR_INFO => self.parse_attr_info_msg(msg_data, attrs)?,
                MSG_OH_CONTINUATION => {
                    let mut mc = Cursor::new(self.data.as_slice(), msg_data_start, self.off_size, self.len_size);
                    let cont_addr = mc.offset()?;
                    let cont_len = mc.length()?;
                    if cont_addr != UNDEF_ADDR {
                        self.parse_dataset_oh_v2_continuation(
                            cont_addr,
                            cont_len,
                            creation_order_tracked,
                            ds,
                            attrs,
                            filters,
                        )?;
                    }
                }
                _ => {}
            }

            c.pos = msg_data_start + msg_size;
        }
        Ok(())
    }

    fn parse_dataset_oh_v2_continuation(
        &self,
        addr: u64,
        len: u64,
        creation_order_tracked: bool,
        ds: &mut DatasetInfo,
        attrs: &mut HashMap<String, AttrValue>,
        filters: &mut Vec<Filter>,
    ) -> Result<()> {
        let offset = self.abs_offset(addr);
        let (start, end) = if offset + 4 <= self.data.len() && &self.data[offset..offset + 4] == b"OCHK"
        {
            (offset + 4, offset + len as usize - 4)
        } else {
            (offset, offset + len as usize)
        };

        let mut c = Cursor::new(&self.data, start, self.off_size, self.len_size);
        self.parse_dataset_oh_v2_messages(
            &mut c,
            end,
            creation_order_tracked,
            ds,
            attrs,
            filters,
        )
    }

    // ---- Data Layout Message ----

    fn parse_layout_msg(&self, data: &[u8]) -> Result<Layout> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let version = c.u8()?;

        match version {
            3 => {
                let layout_class = c.u8()?;
                match layout_class {
                    0 => {
                        // Compact
                        let size = c.u16()? as usize;
                        let d = c.bytes(size)?;
                        Ok(Layout::Compact { data: d.to_vec() })
                    }
                    1 => {
                        // Contiguous
                        let address = c.offset()?;
                        let size = c.length()?;
                        Ok(Layout::Contiguous { address, size })
                    }
                    2 => {
                        // Chunked
                        let ndims = c.u8()? as usize;
                        let address = c.offset()?;
                        let mut chunk_dims = Vec::with_capacity(ndims);
                        // ndims includes the element size as last dim for v3
                        for _ in 0..ndims {
                            chunk_dims.push(c.u32()?);
                        }
                        Ok(Layout::Chunked {
                            address,
                            chunk_dims,
                            filters: Vec::new(),
                        })
                    }
                    _ => Err(SofaError::Unsupported(format!(
                        "Layout class: {}",
                        layout_class
                    ))),
                }
            }
            4 => {
                let layout_class = c.u8()?;
                match layout_class {
                    0 => {
                        // Compact
                        let size = c.u16()? as usize;
                        let d = c.bytes(size)?;
                        Ok(Layout::Compact { data: d.to_vec() })
                    }
                    1 => {
                        // Contiguous
                        let address = c.offset()?;
                        let size = c.length()?;
                        Ok(Layout::Contiguous { address, size })
                    }
                    2 => {
                        // Chunked (v4 uses B-tree v2 for chunk indexing)
                        let _flags = c.u8()?;
                        let ndims = c.u8()? as usize;
                        let _dim_size_encoded = c.u8()?;
                        let mut chunk_dims = Vec::with_capacity(ndims);
                        for _ in 0..ndims {
                            chunk_dims.push(c.u32()?);
                        }
                        let _chunk_indexing_type = c.u8()?;
                        let address = c.offset()?;
                        // There may be additional indexing info depending on type
                        Ok(Layout::Chunked {
                            address,
                            chunk_dims,
                            filters: Vec::new(),
                        })
                    }
                    _ => Err(SofaError::Unsupported(format!(
                        "Layout v4 class: {}",
                        layout_class
                    ))),
                }
            }
            _ => Err(SofaError::Unsupported(format!(
                "Layout version: {}",
                version
            ))),
        }
    }

    // ---- Filter Pipeline Message ----

    fn parse_filter_pipeline_msg(&self, data: &[u8]) -> Result<Vec<Filter>> {
        let mut c = Cursor::new(data, 0, self.off_size, self.len_size);
        let version = c.u8()?;
        let num_filters = c.u8()? as usize;

        if version == 1 {
            c.skip(6)?; // reserved
        }

        let mut filters = Vec::with_capacity(num_filters);
        for _ in 0..num_filters {
            let id = c.u16()?;
            let name_length = if version < 2 || id >= 256 {
                c.u16()? as usize
            } else {
                0
            };
            let flags = c.u16()?;
            let num_client_data = c.u16()? as usize;

            if name_length > 0 {
                c.skip((name_length + 7) & !7)?; // padded name
            }

            let mut client_data = Vec::with_capacity(num_client_data);
            for _ in 0..num_client_data {
                client_data.push(c.u32()?);
            }

            // Pad to 8 bytes for v1
            if version == 1 && num_client_data % 2 != 0 {
                c.skip(4)?;
            }

            filters.push(Filter {
                id,
                _flags: flags,
                _client_data: client_data,
            });
        }

        Ok(filters)
    }

    // ---- Public data reading API ----

    pub fn attribute_string(&self, name: &str) -> Result<String> {
        match self.attributes.get(name) {
            Some(AttrValue::String(s)) => Ok(s.clone()),
            Some(other) => Err(SofaError::TypeMismatch {
                expected: "String".into(),
                got: format!("{:?}", other),
            }),
            None => Err(SofaError::MissingAttribute(name.into())),
        }
    }

    pub fn attribute_f64(&self, name: &str) -> Result<f64> {
        match self.attributes.get(name) {
            Some(AttrValue::Float64(v)) => Ok(*v),
            Some(AttrValue::Float32(v)) => Ok(*v as f64),
            Some(AttrValue::Int32(v)) => Ok(*v as f64),
            Some(other) => Err(SofaError::TypeMismatch {
                expected: "f64".into(),
                got: format!("{:?}", other),
            }),
            None => Err(SofaError::MissingAttribute(name.into())),
        }
    }

    pub fn dimension(&self, name: &str) -> Result<usize> {
        match self.dimensions.get(name) {
            Some(v) => Ok(*v as usize),
            None => {
                // Fallback: look at dataset with this name
                match self.datasets.get(name) {
                    Some(ds) if ds.dims.len() == 1 => Ok(ds.dims[0] as usize),
                    _ => Err(SofaError::MissingDimension(name.into())),
                }
            }
        }
    }

    pub fn read_f32(&self, name: &str) -> Result<Vec<f32>> {
        let ds = self.find_dataset(name)?;
        let raw = self.read_dataset_raw(ds)?;

        match ds.dtype {
            DType::Float32 => Ok(raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect()),
            DType::Float64 => Ok(raw
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()) as f32)
                .collect()),
            _ => Err(SofaError::TypeMismatch {
                expected: "f32".into(),
                got: ds.dtype.to_string(),
            }),
        }
    }

    pub fn read_scalar_f32(&self, name: &str) -> Result<f32> {
        let data = self.read_f32(name)?;
        data.into_iter()
            .next()
            .ok_or_else(|| SofaError::InvalidStructure(format!("Empty dataset: {}", name)))
    }

    fn find_dataset(&self, name: &str) -> Result<&DatasetInfo> {
        // Try exact match first
        if let Some(ds) = self.datasets.get(name) {
            return Ok(ds);
        }

        // Try with dots as path separators (e.g. "Data.IR" might be stored as "Data/IR")
        // But in our case, NetCDF4 stores them as literal dotted names in HDF5
        Err(SofaError::MissingVariable(name.into()))
    }

    fn read_dataset_raw(&self, ds: &DatasetInfo) -> Result<Vec<u8>> {
        match &ds.layout {
            Layout::Compact { data } => Ok(data.clone()),
            Layout::Contiguous { address, size } => {
                if *address == UNDEF_ADDR {
                    // Empty/fill-value dataset
                    let total: u64 = ds.dims.iter().product::<u64>() * ds.dtype.element_size() as u64;
                    return Ok(vec![0u8; total as usize]);
                }
                let off = self.abs_offset(*address);
                let end = off + *size as usize;
                if end > self.data.len() {
                    return Err(SofaError::Truncated {
                        offset: *address,
                        need: *size,
                        have: (self.data.len() - off) as u64,
                    });
                }
                Ok(self.data[off..end].to_vec())
            }
            Layout::Chunked {
                address,
                chunk_dims,
                filters,
            } => self.read_chunked_data(ds, *address, chunk_dims, filters),
        }
    }

    fn read_chunked_data(
        &self,
        ds: &DatasetInfo,
        btree_addr: u64,
        chunk_dims: &[u32],
        filters: &[Filter],
    ) -> Result<Vec<u8>> {
        if btree_addr == UNDEF_ADDR {
            let total: u64 = ds.dims.iter().product::<u64>() * ds.dtype.element_size() as u64;
            return Ok(vec![0u8; total as usize]);
        }

        let elem_size = ds.dtype.element_size();
        let total_elements: u64 = ds.dims.iter().product();
        let total_bytes = total_elements as usize * elem_size;
        let mut output = vec![0u8; total_bytes];

        // Determine chunk element count (exclude the trailing element size dim for v3 layout)
        let ndims = ds.dims.len();
        let chunk_element_dims: Vec<u32> = if chunk_dims.len() > ndims {
            chunk_dims[..ndims].to_vec()
        } else {
            chunk_dims.to_vec()
        };

        // Parse B-tree v1 for chunk index
        self.read_chunks_btree_v1(
            btree_addr,
            &ds.dims,
            &chunk_element_dims,
            elem_size,
            filters,
            &mut output,
        )?;

        Ok(output)
    }

    fn read_chunks_btree_v1(
        &self,
        addr: u64,
        ds_dims: &[u64],
        chunk_dims: &[u32],
        elem_size: usize,
        filters: &[Filter],
        output: &mut [u8],
    ) -> Result<()> {
        let mut c = self.cursor_at(addr);
        let sig = c.bytes(4)?;
        if sig != b"TREE" {
            return Err(SofaError::InvalidStructure("Bad chunk TREE signature".into()));
        }
        let node_type = c.u8()?;
        let node_level = c.u8()?;
        let entries_used = c.u16()?;
        let _left = c.offset()?;
        let _right = c.offset()?;

        if node_type != 1 {
            return Err(SofaError::InvalidStructure(format!(
                "Expected raw data chunk B-tree (type 1), got {}",
                node_type
            )));
        }

        let ndims = ds_dims.len();

        if node_level == 0 {
            // Leaf: each entry has chunk_size(4), filter_mask(4), offset[ndims+1](each 8 bytes), then child address
            for _ in 0..entries_used {
                let chunk_size = c.u32()?;
                let filter_mask = c.u32()?;

                let mut chunk_offset = Vec::with_capacity(ndims);
                for _ in 0..=ndims {
                    // ndims+1 offsets (last is zero)
                    chunk_offset.push(c.u64()?);
                }

                let child_addr = c.offset()?;

                if child_addr != UNDEF_ADDR {
                    let raw_chunk = self.read_raw_bytes(child_addr, chunk_size as usize)?;
                    let decompressed = self.decompress_chunk(&raw_chunk, filters, filter_mask)?;
                    self.copy_chunk_to_output(
                        &decompressed,
                        &chunk_offset[..ndims],
                        ds_dims,
                        chunk_dims,
                        elem_size,
                        output,
                    );
                }
            }
        } else {
            // Internal node: keys are chunk offsets, children are subtree addresses
            for _ in 0..entries_used {
                let _chunk_size = c.u32()?;
                let _filter_mask = c.u32()?;
                for _ in 0..=ndims {
                    let _offset = c.u64()?;
                }
                let child_addr = c.offset()?;
                if child_addr != UNDEF_ADDR {
                    self.read_chunks_btree_v1(
                        child_addr,
                        ds_dims,
                        chunk_dims,
                        elem_size,
                        filters,
                        output,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn read_raw_bytes(&self, addr: u64, size: usize) -> Result<Vec<u8>> {
        let off = self.abs_offset(addr);
        if off + size > self.data.len() {
            return Err(SofaError::Truncated {
                offset: addr,
                need: size as u64,
                have: (self.data.len() - off) as u64,
            });
        }
        Ok(self.data[off..off + size].to_vec())
    }

    fn decompress_chunk(
        &self,
        data: &[u8],
        filters: &[Filter],
        filter_mask: u32,
    ) -> Result<Vec<u8>> {
        let mut buf = data.to_vec();

        // Apply filters in reverse order
        for (i, filter) in filters.iter().enumerate().rev() {
            if filter_mask & (1 << i) != 0 {
                continue; // This filter was skipped
            }
            match filter.id {
                1 => {
                    // Deflate
                    #[cfg(feature = "deflate")]
                    {
                        use flate2::read::ZlibDecoder;
                        use std::io::Read;
                        let mut decoder = ZlibDecoder::new(&buf[..]);
                        let mut decompressed = Vec::new();
                        decoder
                            .read_to_end(&mut decompressed)
                            .map_err(|e| SofaError::InvalidStructure(format!("Deflate error: {}", e)))?;
                        buf = decompressed;
                    }
                    #[cfg(not(feature = "deflate"))]
                    {
                        return Err(SofaError::Unsupported(
                            "Deflate compression (enable 'deflate' feature)".into(),
                        ));
                    }
                }
                2 => {
                    // Shuffle: de-shuffle bytes
                    let elem_size = if !filter._client_data.is_empty() {
                        filter._client_data[0] as usize
                    } else {
                        1
                    };
                    if elem_size > 1 {
                        let n = buf.len() / elem_size;
                        let mut unshuffled = vec![0u8; buf.len()];
                        for i in 0..n {
                            for j in 0..elem_size {
                                unshuffled[i * elem_size + j] = buf[j * n + i];
                            }
                        }
                        buf = unshuffled;
                    }
                }
                3 => {
                    // Fletcher32 checksum - just strip last 4 bytes
                    if buf.len() >= 4 {
                        buf.truncate(buf.len() - 4);
                    }
                }
                _ => {
                    return Err(SofaError::Unsupported(format!(
                        "Filter ID: {}",
                        filter.id
                    )));
                }
            }
        }

        Ok(buf)
    }

    fn copy_chunk_to_output(
        &self,
        chunk_data: &[u8],
        chunk_offset: &[u64],
        ds_dims: &[u64],
        chunk_dims: &[u32],
        elem_size: usize,
        output: &mut [u8],
    ) {
        let ndims = ds_dims.len();

        if ndims == 0 {
            // Scalar
            let n = chunk_data.len().min(output.len());
            output[..n].copy_from_slice(&chunk_data[..n]);
            return;
        }

        // For multi-dimensional: compute strides and copy
        // Output strides (row-major)
        let mut out_strides = vec![1usize; ndims];
        for i in (0..ndims - 1).rev() {
            out_strides[i] = out_strides[i + 1] * ds_dims[i + 1] as usize;
        }

        // Chunk strides
        let mut chunk_strides = vec![1usize; ndims];
        for i in (0..ndims - 1).rev() {
            chunk_strides[i] = chunk_strides[i + 1] * chunk_dims[i + 1] as usize;
        }

        // Iterate over chunk elements
        let chunk_total: usize = chunk_dims.iter().map(|&d| d as usize).product();
        for flat_idx in 0..chunk_total {
            // Convert flat index to multi-dim index within chunk
            let mut remaining = flat_idx;
            let mut in_bounds = true;

            let mut out_flat = 0usize;
            for d in 0..ndims {
                let local_idx = remaining / chunk_strides[d];
                remaining %= chunk_strides[d];

                let global_idx = chunk_offset[d] as usize + local_idx;
                if global_idx >= ds_dims[d] as usize {
                    in_bounds = false;
                    break;
                }
                out_flat += global_idx * out_strides[d];
            }

            if in_bounds {
                let src_start = flat_idx * elem_size;
                let dst_start = out_flat * elem_size;
                if src_start + elem_size <= chunk_data.len()
                    && dst_start + elem_size <= output.len()
                {
                    output[dst_start..dst_start + elem_size]
                        .copy_from_slice(&chunk_data[src_start..src_start + elem_size]);
                }
            }
        }
    }

    // ---- Attribute access by name with "Variable:Attribute" convention ----

    pub fn attribute(&self, name: &str) -> Option<&AttrValue> {
        self.attributes.get(name)
    }

    pub fn has_dataset(&self, name: &str) -> bool {
        self.datasets.contains_key(name)
    }

    pub fn dataset_dims(&self, name: &str) -> Result<Vec<u64>> {
        let ds = self.find_dataset(name)?;
        Ok(ds.dims.clone())
    }
}
