#[derive(Clone, Default)]
pub(crate) struct ComputeSegment {
    out_offsets: Vec<usize>,
    out_edges: Vec<u64>,
    in_offsets: Vec<usize>,
    in_edges: Vec<u64>,
}

impl ComputeSegment {
    pub(super) fn from_adjacency(out_adj: &[Vec<u64>], in_adj: &[Vec<u64>]) -> Self {
        let (out_offsets, out_edges) = flatten_adjacency(out_adj);
        let (in_offsets, in_edges) = flatten_adjacency(in_adj);
        Self {
            out_offsets,
            out_edges,
            in_offsets,
            in_edges,
        }
    }

    pub(crate) fn from_bytes(
        bytes: &[u8],
        expected_nodes: usize,
        expected_edges: usize,
    ) -> Result<Self, String> {
        let mut cursor = Cursor::new(bytes);
        cursor.read_magic()?;
        let node_count = cursor.read_usize("node count")?;
        if node_count != expected_nodes {
            return Err(format!(
                "segment node count {node_count} does not match expected {expected_nodes}"
            ));
        }
        let out_offsets = cursor.read_usize_vec("out offsets")?;
        let out_edges = cursor.read_u64_vec("out edges")?;
        let in_offsets = cursor.read_usize_vec("in offsets")?;
        let in_edges = cursor.read_u64_vec("in edges")?;
        if cursor.remaining() != 0 {
            return Err("segment file has trailing bytes".to_string());
        }

        let segment = Self {
            out_offsets,
            out_edges,
            in_offsets,
            in_edges,
        };
        segment.validate(expected_nodes, expected_edges)?;
        Ok(segment)
    }

    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SEGMENT_MAGIC);
        write_u64(&mut bytes, self.node_count()? as u64);
        write_usize_vec(&mut bytes, &self.out_offsets)?;
        write_u64_vec(&mut bytes, &self.out_edges);
        write_usize_vec(&mut bytes, &self.in_offsets)?;
        write_u64_vec(&mut bytes, &self.in_edges);
        Ok(bytes)
    }

    pub(crate) fn edge_count(&self) -> usize {
        self.out_edges.len()
    }

    pub(super) fn out_edges(&self, node_id: usize) -> &[u64] {
        self.edges_for_node(node_id, &self.out_offsets, &self.out_edges)
    }

    pub(super) fn in_edges(&self, node_id: usize) -> &[u64] {
        self.edges_for_node(node_id, &self.in_offsets, &self.in_edges)
    }

    fn edges_for_node<'a>(&self, node_id: usize, offsets: &[usize], edges: &'a [u64]) -> &'a [u64] {
        if node_id + 1 >= offsets.len() {
            return &[];
        }
        let start = offsets[node_id];
        let end = offsets[node_id + 1];
        &edges[start..end]
    }

    fn node_count(&self) -> Result<usize, String> {
        if self.out_offsets.is_empty() {
            return Ok(0);
        }
        Ok(self.out_offsets.len() - 1)
    }

    fn validate(&self, expected_nodes: usize, expected_edges: usize) -> Result<(), String> {
        validate_offsets(
            "out",
            &self.out_offsets,
            self.out_edges.len(),
            expected_nodes,
        )?;
        validate_offsets("in", &self.in_offsets, self.in_edges.len(), expected_nodes)?;
        if self.out_edges.len() != expected_edges {
            return Err(format!(
                "segment outgoing edge count {} does not match expected {expected_edges}",
                self.out_edges.len()
            ));
        }
        if self.in_edges.len() != expected_edges {
            return Err(format!(
                "segment incoming edge count {} does not match expected {expected_edges}",
                self.in_edges.len()
            ));
        }
        Ok(())
    }
}

fn flatten_adjacency(adjacency: &[Vec<u64>]) -> (Vec<usize>, Vec<u64>) {
    let mut offsets = Vec::with_capacity(adjacency.len() + 1);
    let mut edges = Vec::new();
    offsets.push(0);
    for edge_ids in adjacency {
        edges.extend(edge_ids);
        offsets.push(edges.len());
    }
    (offsets, edges)
}

const SEGMENT_MAGIC: &[u8; 8] = b"TGSEG001";

fn validate_offsets(
    name: &str,
    offsets: &[usize],
    edge_len: usize,
    node_count: usize,
) -> Result<(), String> {
    if offsets.len() != node_count + 1 {
        return Err(format!(
            "{name} offsets length {} does not match node count {node_count}",
            offsets.len()
        ));
    }
    if offsets.first().copied() != Some(0) {
        return Err(format!("{name} offsets must start at 0"));
    }
    if offsets.last().copied() != Some(edge_len) {
        return Err(format!(
            "{name} offsets end {} does not match edge length {edge_len}",
            offsets.last().copied().unwrap_or_default()
        ));
    }
    for pair in offsets.windows(2) {
        if pair[0] > pair[1] {
            return Err(format!("{name} offsets must be non-decreasing"));
        }
    }
    Ok(())
}

fn write_usize_vec(bytes: &mut Vec<u8>, values: &[usize]) -> Result<(), String> {
    write_u64(bytes, values.len() as u64);
    for value in values {
        let value = u64::try_from(*value)
            .map_err(|_| format!("segment offset {value} exceeds u64 range"))?;
        write_u64(bytes, value);
    }
    Ok(())
}

fn write_u64_vec(bytes: &mut Vec<u8>, values: &[u64]) {
    write_u64(bytes, values.len() as u64);
    for value in values {
        write_u64(bytes, *value);
    }
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_magic(&mut self) -> Result<(), String> {
        let magic = self.read_exact(SEGMENT_MAGIC.len(), "magic")?;
        if magic != SEGMENT_MAGIC {
            return Err("invalid segment file magic".to_string());
        }
        Ok(())
    }

    fn read_usize(&mut self, field: &str) -> Result<usize, String> {
        let value = self.read_u64(field)?;
        usize::try_from(value).map_err(|_| format!("{field} {value} exceeds usize range"))
    }

    fn read_usize_vec(&mut self, field: &str) -> Result<Vec<usize>, String> {
        let len = self.read_usize(field)?;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(self.read_usize(field)?);
        }
        Ok(values)
    }

    fn read_u64_vec(&mut self, field: &str) -> Result<Vec<u64>, String> {
        let len = self.read_usize(field)?;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(self.read_u64(field)?);
        }
        Ok(values)
    }

    fn read_u64(&mut self, field: &str) -> Result<u64, String> {
        let bytes = self.read_exact(8, field)?;
        let mut value = [0u8; 8];
        value.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(value))
    }

    fn read_exact(&mut self, len: usize, field: &str) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| format!("segment {field} length overflows"))?;
        if end > self.bytes.len() {
            return Err(format!("segment file ended while reading {field}"));
        }
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}
