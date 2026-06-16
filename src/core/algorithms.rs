use super::adjacency::edge_weight;
use super::direction::Direction;
use super::GraphCore;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShortestPath {
    pub(crate) nodes: Vec<u64>,
    pub(crate) distance: f64,
}

#[derive(Clone, Debug)]
pub(crate) enum ComputeJob {
    Bfs {
        start: u64,
        direction: String,
        edge_type: Option<String>,
        max_depth: Option<usize>,
    },
    ShortestPath {
        start: u64,
        target: u64,
        direction: String,
        edge_type: Option<String>,
        weight_property: Option<String>,
    },
    ConnectedComponents {
        edge_type: Option<String>,
    },
    PageRank {
        iterations: usize,
        damping: f64,
        tolerance: Option<f64>,
        edge_type: Option<String>,
    },
    RandomWalk {
        start: u64,
        steps: usize,
        direction: String,
        edge_type: Option<String>,
        seed: Option<u64>,
    },
    Subgraph {
        nodes: Vec<u64>,
        edge_type: Option<String>,
    },
}

pub(crate) enum ComputeResult {
    Nodes(Vec<u64>),
    ShortestPath(Option<ShortestPath>),
    Components(Vec<Vec<u64>>),
    Scores(BTreeMap<u64, f64>),
    Snapshot(GraphCore),
}

impl GraphCore {
    pub(crate) fn bfs(
        &self,
        start: u64,
        direction: &str,
        edge_type: Option<&str>,
        max_depth: Option<usize>,
    ) -> Result<Vec<u64>, String> {
        self.require_node(start)?;
        let direction = Direction::parse(direction)?;
        let mut visited = BTreeSet::new();
        let mut ordered = Vec::new();
        let mut frontier = VecDeque::from([(start, 0usize)]);
        visited.insert(start);
        ordered.push(start);

        while let Some((node_id, depth)) = frontier.pop_front() {
            if max_depth.is_some_and(|limit| depth >= limit) {
                continue;
            }
            for next in self.neighbors_for_direction(node_id, direction, edge_type) {
                if visited.insert(next) {
                    ordered.push(next);
                    frontier.push_back((next, depth + 1));
                }
            }
        }

        Ok(ordered)
    }

    pub(crate) fn shortest_path(
        &self,
        start: u64,
        target: u64,
        direction: &str,
        edge_type: Option<&str>,
        weight_property: Option<&str>,
    ) -> Result<Option<ShortestPath>, String> {
        self.require_node(start)?;
        self.require_node(target)?;
        if start == target {
            return Ok(Some(ShortestPath {
                nodes: vec![start],
                distance: 0.0,
            }));
        }

        let direction = Direction::parse(direction)?;
        let mut distances = HashMap::from([(start, 0.0)]);
        let mut previous = HashMap::new();
        let mut heap = BinaryHeap::from([PathState {
            distance: 0.0,
            node: start,
        }]);

        while let Some(PathState { distance, node }) = heap.pop() {
            if distance > *distances.get(&node).unwrap_or(&f64::INFINITY) {
                continue;
            }
            if node == target {
                break;
            }

            for (next, edge_id) in self.neighbor_edges_for_direction(node, direction, edge_type) {
                let edge = self
                    .edge_record(edge_id)
                    .ok_or_else(|| format!("edge {edge_id} not found"))?;
                let weight = match weight_property {
                    Some(property) => edge_weight(edge, property)?,
                    None => 1.0,
                };
                let next_distance = distance + weight;
                if next_distance < *distances.get(&next).unwrap_or(&f64::INFINITY) {
                    distances.insert(next, next_distance);
                    previous.insert(next, node);
                    heap.push(PathState {
                        distance: next_distance,
                        node: next,
                    });
                }
            }
        }

        let Some(&distance) = distances.get(&target) else {
            return Ok(None);
        };
        let mut nodes = vec![target];
        let mut current = target;
        while current != start {
            let Some(&parent) = previous.get(&current) else {
                return Ok(None);
            };
            current = parent;
            nodes.push(current);
        }
        nodes.reverse();
        Ok(Some(ShortestPath { nodes, distance }))
    }

    pub(crate) fn connected_components(
        &self,
        edge_type: Option<&str>,
    ) -> Result<Vec<Vec<u64>>, String> {
        let mut visited = BTreeSet::new();
        let mut components = Vec::new();

        for start in self.existing_node_ids() {
            if visited.contains(&start) {
                continue;
            }
            let mut component = Vec::new();
            let mut frontier = VecDeque::from([start]);
            visited.insert(start);

            while let Some(node_id) = frontier.pop_front() {
                component.push(node_id);
                for next in self.neighbors_for_direction(node_id, Direction::Both, edge_type) {
                    if visited.insert(next) {
                        frontier.push_back(next);
                    }
                }
            }
            components.push(component);
        }

        Ok(components)
    }

    pub(crate) fn pagerank(
        &self,
        iterations: usize,
        damping: f64,
        tolerance: Option<f64>,
        edge_type: Option<&str>,
    ) -> Result<BTreeMap<u64, f64>, String> {
        if !(0.0..=1.0).contains(&damping) {
            return Err("damping must be between 0.0 and 1.0".to_string());
        }
        if let Some(tolerance) = tolerance {
            if !tolerance.is_finite() || tolerance < 0.0 {
                return Err("tolerance must be finite and non-negative".to_string());
            }
        }

        let nodes = self.existing_node_ids();
        if nodes.is_empty() {
            return Ok(BTreeMap::new());
        }

        let node_count = nodes.len() as f64;
        let initial = 1.0 / node_count;
        let mut ranks = nodes
            .iter()
            .copied()
            .map(|node_id| (node_id, initial))
            .collect::<BTreeMap<_, _>>();

        for _ in 0..iterations {
            let dangling_rank = nodes
                .iter()
                .copied()
                .filter(|&node_id| {
                    self.neighbors_for_direction(node_id, Direction::Out, edge_type)
                        .is_empty()
                })
                .map(|node_id| ranks[&node_id])
                .sum::<f64>();
            let base = (1.0 - damping) / node_count + damping * dangling_rank / node_count;
            let mut next = nodes
                .iter()
                .copied()
                .map(|node_id| (node_id, base))
                .collect::<BTreeMap<_, _>>();

            for &source in &nodes {
                let targets = self.neighbors_for_direction(source, Direction::Out, edge_type);
                if targets.is_empty() {
                    continue;
                }
                let contribution = damping * ranks[&source] / targets.len() as f64;
                for target in targets {
                    if let Some(rank) = next.get_mut(&target) {
                        *rank += contribution;
                    }
                }
            }

            let diff = nodes
                .iter()
                .map(|node_id| (next[node_id] - ranks[node_id]).abs())
                .sum::<f64>();
            ranks = next;
            if tolerance.is_some_and(|limit| diff <= limit) {
                break;
            }
        }

        Ok(ranks)
    }

    pub(crate) fn random_walk(
        &self,
        start: u64,
        steps: usize,
        direction: &str,
        edge_type: Option<&str>,
        seed: Option<u64>,
    ) -> Result<Vec<u64>, String> {
        self.require_node(start)?;
        let direction = Direction::parse(direction)?;
        let mut rng = SmallRng::new(seed.unwrap_or_else(runtime_seed));
        let mut path = Vec::with_capacity(steps + 1);
        let mut current = start;
        path.push(current);

        for _ in 0..steps {
            let neighbors = self.neighbors_for_direction(current, direction, edge_type);
            if neighbors.is_empty() {
                break;
            }
            current = neighbors[rng.next_index(neighbors.len())];
            path.push(current);
        }

        Ok(path)
    }

    pub(crate) fn subgraph(&self, nodes: &[u64], edge_type: Option<&str>) -> Result<Self, String> {
        let mut selected = BTreeSet::new();
        for &node_id in nodes {
            self.require_node(node_id)?;
            selected.insert(node_id);
        }

        let mut subgraph = Self::new();
        for &node_id in &selected {
            let node = self
                .get_node(node_id)
                .ok_or_else(|| format!("node {node_id} not found"))?;
            subgraph.insert_loaded_node(node)?;
        }
        for edge in self.edges.iter().flatten() {
            if !selected.contains(&edge.source) || !selected.contains(&edge.target) {
                continue;
            }
            if let Some(filter) = edge_type {
                if edge.edge_type != filter {
                    continue;
                }
            }
            subgraph.insert_loaded_edge(edge.clone())?;
        }
        subgraph.compact_segments()?;
        Ok(subgraph)
    }

    pub(crate) fn compute_batch(&self, jobs: &[ComputeJob]) -> Result<Vec<ComputeResult>, String> {
        let mut results = Vec::with_capacity(jobs.len());
        for (index, job) in jobs.iter().enumerate() {
            let result = match job {
                ComputeJob::Bfs {
                    start,
                    direction,
                    edge_type,
                    max_depth,
                } => ComputeResult::Nodes(
                    self.bfs(*start, direction, edge_type.as_deref(), *max_depth)
                        .map_err(|error| format!("job {index}: {error}"))?,
                ),
                ComputeJob::ShortestPath {
                    start,
                    target,
                    direction,
                    edge_type,
                    weight_property,
                } => ComputeResult::ShortestPath(
                    self.shortest_path(
                        *start,
                        *target,
                        direction,
                        edge_type.as_deref(),
                        weight_property.as_deref(),
                    )
                    .map_err(|error| format!("job {index}: {error}"))?,
                ),
                ComputeJob::ConnectedComponents { edge_type } => ComputeResult::Components(
                    self.connected_components(edge_type.as_deref())
                        .map_err(|error| format!("job {index}: {error}"))?,
                ),
                ComputeJob::PageRank {
                    iterations,
                    damping,
                    tolerance,
                    edge_type,
                } => ComputeResult::Scores(
                    self.pagerank(*iterations, *damping, *tolerance, edge_type.as_deref())
                        .map_err(|error| format!("job {index}: {error}"))?,
                ),
                ComputeJob::RandomWalk {
                    start,
                    steps,
                    direction,
                    edge_type,
                    seed,
                } => ComputeResult::Nodes(
                    self.random_walk(*start, *steps, direction, edge_type.as_deref(), *seed)
                        .map_err(|error| format!("job {index}: {error}"))?,
                ),
                ComputeJob::Subgraph { nodes, edge_type } => ComputeResult::Snapshot(
                    self.subgraph(nodes, edge_type.as_deref())
                        .map_err(|error| format!("job {index}: {error}"))?,
                ),
            };
            results.push(result);
        }
        Ok(results)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PathState {
    distance: f64,
    node: u64,
}

impl Eq for PathState {}

impl Ord for PathState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .distance
            .total_cmp(&self.distance)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for PathState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct SmallRng {
    state: u64,
}

impl SmallRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_index(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state ^ (self.state >> 33)
    }
}

fn runtime_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
}
