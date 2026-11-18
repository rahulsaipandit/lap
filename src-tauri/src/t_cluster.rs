use crate::t_common;
use crate::t_sqlite::{Face, Person};
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};

/// Clustering progress information
#[derive(Clone, serde::Serialize)]
pub struct ClusterProgress {
    pub phase: String, // "anchor", "graph", "iterate", "converged", "assign", "thumbnail"
    pub current: usize,
    pub total: usize,
}

/// Maximum number of reference embeddings compared per locked person when attaching new faces.
/// Bounds the cost of attaching to O(free faces x locked persons x MAX_ANCHOR_SAMPLES).
const MAX_ANCHOR_SAMPLES: usize = 200;

/// Calculate cosine distance between two PRE-PARSED embeddings
/// Distance = 1.0 - Cosine Similarity
/// NOTE: Assumes input vectors are already normalized!
fn cosine_distance(emb1: &[f32], emb2: &[f32]) -> f32 {
    // Dot product of normalized vectors = cosine similarity
    let dot_product: f32 = emb1.iter().zip(emb2.iter()).map(|(x, y)| x * y).sum();

    // Clamp similarity to [-1.0, 1.0] to handle floating point errors
    let similarity = dot_product.clamp(-1.0, 1.0);
    1.0 - similarity
}

/// Helper: Parse raw byte embedding to normalized f32 vector
fn parse_embedding(bytes: &[u8]) -> Option<Vec<f32>> {
    // Embedding bytes should be tightly packed f32 values.
    // If not, skip this embedding instead of panicking.
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }
    let emb_vec: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect();

    // Normalize
    let norm: f32 = emb_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        Some(emb_vec.iter().map(|x| x / norm).collect())
    } else {
        None
    }
}

/// Edge in the similarity graph
#[derive(Clone)]
struct Edge {
    to: usize,   // Target node index
    weight: f32, // Edge weight (similarity = 1 - distance)
}

/// Insert edge into candidate list, maintaining at most max_edges entries sorted by weight descending.
/// If list is full and new weight is smaller than the smallest, it is dropped.
fn insert_top_k(candidates: &mut Vec<(usize, f32)>, edge: (usize, f32), max_edges: usize) {
    if candidates.len() < max_edges {
        candidates.push(edge);
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    } else {
        // Find the smallest weight (last element after sort)
        if let Some(&(_, min_weight)) = candidates.last() {
            if edge.1 > min_weight {
                candidates.pop();
                candidates.push(edge);
                candidates
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }
}

/// A face that is not (yet) attached to a locked person.
pub(crate) struct FreeFace {
    pub file_id: i64,
    pub emb: Option<Vec<f32>>,
}

/// Reference data for the locked ("anchor") persons.
#[derive(Default)]
pub(crate) struct AnchorSet {
    /// (person_id, file_id) for EVERY face of a locked person. Used to enforce that one photo
    /// cannot contain the same person twice, even for faces without a usable embedding.
    pub members: Vec<(i64, i64)>,
    /// (person_id, embedding) reference vectors, at most MAX_ANCHOR_SAMPLES per person.
    pub samples: Vec<(i64, Vec<f32>)>,
}

/// Evenly sample at most `max` items from `items`, deterministically.
fn stride_sample(items: Vec<Vec<f32>>, max: usize) -> Vec<Vec<f32>> {
    if items.len() <= max {
        return items;
    }
    let len = items.len();
    items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| (i * max) / len != ((i + 1) * max) / len)
        .map(|(_, v)| v)
        .collect()
}

/// Attach free faces to the nearest locked person.
///
/// A face attaches to a locked person when its distance to the closest reference embedding of
/// that person is below `threshold`. A face is never attached to a person who already has a face
/// in the same photo (two faces in one photo are different people). Faces with the smallest
/// distance are placed first so the best matches win when a photo has several candidates.
///
/// Returns, for each free face (same index), the person it attached to, if any.
pub(crate) fn attach_to_anchors(
    free: &[FreeFace],
    anchors: &AnchorSet,
    threshold: f32,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Option<i64>>> {
    let mut result: Vec<Option<i64>> = vec![None; free.len()];
    if anchors.samples.is_empty() {
        return Some(result);
    }

    // Candidate persons per free face, nearest first.
    let mut candidates: Vec<Vec<(f32, i64)>> = vec![Vec::new(); free.len()];
    for (i, face) in free.iter().enumerate() {
        if i % 256 == 0 && is_cancelled() {
            return None;
        }
        let Some(emb) = &face.emb else { continue };
        let mut best: HashMap<i64, f32> = HashMap::new();
        for (person_id, reference) in &anchors.samples {
            let dist = cosine_distance(emb, reference);
            if dist < threshold {
                let entry = best.entry(*person_id).or_insert(dist);
                if dist < *entry {
                    *entry = dist;
                }
            }
        }
        let mut list: Vec<(f32, i64)> = best.into_iter().map(|(p, d)| (d, p)).collect();
        list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
        candidates[i] = list;
    }

    // Place the most confident faces first.
    let mut order: Vec<usize> = (0..free.len()).filter(|i| !candidates[*i].is_empty()).collect();
    order.sort_by(|a, b| {
        candidates[*a][0]
            .0
            .partial_cmp(&candidates[*b][0].0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });

    let mut person_files: HashSet<(i64, i64)> = anchors.members.iter().cloned().collect();
    for i in order {
        let file_id = free[i].file_id;
        if let Some((_, person_id)) = candidates[i]
            .iter()
            .find(|(_, p)| !person_files.contains(&(*p, file_id)))
        {
            result[i] = Some(*person_id);
            person_files.insert((*person_id, file_id));
        }
    }
    Some(result)
}

/// Match new clusters to existing (unlocked) persons so their rows, and therefore ids, covers and
/// default names, stay stable between runs.
///
/// `old_person_of` maps face id -> previous person id; `old_size` is the previous size of each
/// person. Matching is greedy by overlap size and one-to-one. A pair only matches when the overlap
/// covers at least half of the smaller side. Ties are broken by ids for determinism.
///
/// Returns, per cluster, the person it keeps.
pub(crate) fn match_clusters_to_persons(
    clusters: &[Vec<i64>],
    old_person_of: &HashMap<i64, i64>,
    old_size: &HashMap<i64, usize>,
) -> Vec<Option<i64>> {
    let mut candidates: Vec<(usize, usize, i64)> = Vec::new(); // (overlap, cluster_idx, person_id)
    for (ci, faces) in clusters.iter().enumerate() {
        let mut counts: HashMap<i64, usize> = HashMap::new();
        for fid in faces {
            if let Some(pid) = old_person_of.get(fid) {
                *counts.entry(*pid).or_insert(0) += 1;
            }
        }
        for (pid, overlap) in counts {
            let smaller = faces.len().min(*old_size.get(&pid).unwrap_or(&0));
            if overlap * 2 >= smaller {
                candidates.push((overlap, ci, pid));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)));

    let mut cluster_person: Vec<Option<i64>> = vec![None; clusters.len()];
    let mut used: HashSet<i64> = HashSet::new();
    for (_, ci, pid) in candidates {
        if cluster_person[ci].is_none() && !used.contains(&pid) {
            cluster_person[ci] = Some(pid);
            used.insert(pid);
        }
    }
    cluster_person
}

/// Next free number for default names ("Person N"): one above the largest N in `names`.
pub(crate) fn next_person_number<'a>(names: impl Iterator<Item = &'a str>) -> usize {
    let mut next = 1;
    for name in names {
        if let Some(n) = name
            .strip_prefix("Person ")
            .and_then(|s| s.trim().parse::<usize>().ok())
        {
            next = next.max(n + 1);
        }
    }
    next
}

/// Chinese Whispers over a thresholded K-NN graph.
///
/// `faces` is (face_id, file_id) and `embeddings` is index-aligned with it. Returns the clusters
/// as face-id lists, or `None` if cancelled.
///
/// Memory-optimized: prunes candidate edges to Top-K during build (not after), bounding memory at
/// N * K_NEIGHBORS.
fn chinese_whispers<F, C>(
    faces: &[(i64, i64)],
    embeddings: Vec<Option<Vec<f32>>>,
    threshold: f32,
    progress_fn: &mut F,
    is_cancelled_fn: &C,
) -> Option<Vec<Vec<i64>>>
where
    F: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    const K_NEIGHBORS: usize = t_common::K_NEIGHBORS;
    let n = faces.len();
    if n == 0 {
        return Some(Vec::new());
    }
    let parsed_embeddings = embeddings;

    // Build K-NN Graph with early Top-K pruning
    let mut candidate_lists: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    let total_pairs = n.saturating_mul(n.saturating_sub(1)) / 2;
    let mut pairs_done: usize = 0;
    let mut last_pct = 0;

    for i in 0..n {
        if is_cancelled_fn() {
            return None;
        }

        if let Some(emb_i) = &parsed_embeddings[i] {
            for j in (i + 1)..n {
                if let Some(emb_j) = &parsed_embeddings[j] {
                    // Faces in the same file cannot be edges (prevents merging distinct people in same photo)
                    if faces[i].1 == faces[j].1 {
                        pairs_done += 1;
                        continue;
                    }

                    let dist = cosine_distance(emb_i, emb_j);

                    if dist < threshold {
                        let weight = 1.0 - dist;
                        // Square weight to punish weak links further
                        let adjusted_weight = weight * weight;

                        insert_top_k(&mut candidate_lists[i], (j, adjusted_weight), K_NEIGHBORS);
                        insert_top_k(&mut candidate_lists[j], (i, adjusted_weight), K_NEIGHBORS);
                    }
                }
                pairs_done += 1;
            }
        } else {
            pairs_done += n.saturating_sub(i + 1);
        }

        // Report progress every 5%
        let current_pct = if total_pairs > 0 {
            pairs_done * 100 / total_pairs
        } else {
            100
        };
        if current_pct >= last_pct + 5 || pairs_done == total_pairs {
            progress_fn(ClusterProgress {
                phase: "graph".to_string(),
                current: current_pct,
                total: 100,
            });
            last_pct = current_pct;
        }
    }

    // Build final graph from pruned candidate lists (edges already Top-K)
    let mut graph: Vec<Vec<Edge>> = vec![Vec::new(); n];
    for (i, candidates) in candidate_lists.iter().enumerate() {
        for &(to, weight) in candidates {
            graph[i].push(Edge { to, weight });
        }
    }

    // Free graph-building allocations before Chinese Whispers
    drop(candidate_lists);
    drop(parsed_embeddings);

    // Run Chinese Whispers Algorithm
    let mut labels: Vec<usize> = (0..n).collect();
    let mut order: Vec<usize> = (0..n).collect();
    let mut rng = rand::thread_rng();
    let max_iterations = 20;

    for iter in 0..max_iterations {
        if is_cancelled_fn() {
            return None;
        }

        let mut changed = false;

        progress_fn(ClusterProgress {
            phase: "iterate".to_string(),
            current: iter + 1,
            total: max_iterations,
        });

        order.shuffle(&mut rng);

        for &node in &order {
            if graph[node].is_empty() {
                continue;
            }

            // Count weighted votes
            let mut label_weights: HashMap<usize, f32> = HashMap::new();
            for edge in &graph[node] {
                let neighbor_label = labels[edge.to];
                *label_weights.entry(neighbor_label).or_insert(0.0) += edge.weight;
            }

            // Find best label
            let best_label = label_weights
                .into_iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(label, _)| label)
                .unwrap_or(labels[node]);

            if labels[node] != best_label {
                labels[node] = best_label;
                changed = true;
            }
        }

        if !changed {
            progress_fn(ClusterProgress {
                phase: "converged".to_string(),
                current: iter + 1,
                total: max_iterations,
            });
            break;
        }
    }

    if is_cancelled_fn() {
        return None;
    }

    // Collect clusters
    let mut cluster_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, &label) in labels.iter().enumerate() {
        cluster_map.entry(label).or_default().push(i);
    }

    // Filter clusters
    const MIN_SAMPLES: usize = t_common::MIN_SAMPLES;
    Some(
        cluster_map
            .into_values()
            .filter(|indices| indices.len() >= MIN_SAMPLES)
            .map(|indices| indices.into_iter().map(|i| faces[i].0).collect())
            .collect(),
    )
}

/// Re-cluster faces while keeping people the user has named.
///
/// * Faces of **locked** persons (renamed by the user) stay exactly where they are. They act as
///   anchors.
/// * Every other face is first attached to the nearest anchor person if it is within `threshold`.
/// * Whatever is left is clustered with Chinese Whispers and matched to the previous automatic
///   persons by overlap so their ids stay stable.
/// * Ignored faces (from deleted persons) are skipped entirely.
///
/// Nothing is written until the end, in a single transaction, so a cancel or error leaves the
/// previous result intact.
pub fn cluster_faces<F, C>(
    threshold: f32,
    mut progress_fn: F,
    is_cancelled_fn: C,
) -> Result<usize, String>
where
    F: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    // 1. Load faces and split them into anchors (locked persons) and free faces.
    let faces = Face::get_for_clustering()?;
    if faces.is_empty() {
        // No faces left: just drop persons that no longer have any.
        Person::apply_clusters(&[], &[])?;
        return Ok(0);
    }

    let mut anchor_members: Vec<(i64, i64)> = Vec::new();
    let mut anchor_embs: HashMap<i64, Vec<Vec<f32>>> = HashMap::new();
    let mut free_meta: Vec<(i64, i64)> = Vec::new(); // (face_id, file_id)
    let mut free_faces: Vec<FreeFace> = Vec::new();

    for face in faces {
        let emb = face.embedding.as_deref().and_then(parse_embedding);
        match face.locked_person {
            Some(person_id) => {
                anchor_members.push((person_id, face.file_id));
                if let Some(e) = emb {
                    anchor_embs.entry(person_id).or_default().push(e);
                }
            }
            None => {
                free_meta.push((face.id, face.file_id));
                free_faces.push(FreeFace {
                    file_id: face.file_id,
                    emb,
                });
            }
        }
    }

    let mut samples: Vec<(i64, Vec<f32>)> = Vec::new();
    let mut person_ids: Vec<i64> = anchor_embs.keys().cloned().collect();
    person_ids.sort_unstable(); // deterministic
    for person_id in person_ids {
        if let Some(embs) = anchor_embs.remove(&person_id) {
            for e in stride_sample(embs, MAX_ANCHOR_SAMPLES) {
                samples.push((person_id, e));
            }
        }
    }
    let anchors = AnchorSet {
        members: anchor_members,
        samples,
    };

    // 2. Attach free faces to locked persons.
    progress_fn(ClusterProgress {
        phase: "anchor".to_string(),
        current: 0,
        total: free_faces.len(),
    });
    let attached = match attach_to_anchors(&free_faces, &anchors, threshold, &is_cancelled_fn) {
        Some(a) => a,
        None => return Ok(0),
    };
    drop(anchors);

    let mut anchored: Vec<(i64, i64)> = Vec::new(); // (face_id, person_id)
    let mut rest_meta: Vec<(i64, i64)> = Vec::new();
    let mut rest_embs: Vec<Option<Vec<f32>>> = Vec::new();
    for (i, face) in free_faces.into_iter().enumerate() {
        match attached[i] {
            Some(person_id) => anchored.push((free_meta[i].0, person_id)),
            None => {
                rest_meta.push(free_meta[i]);
                rest_embs.push(face.emb);
            }
        }
    }
    drop(attached);
    drop(free_meta);

    // 3. Cluster the remaining faces among themselves.
    let clusters = match chinese_whispers(
        &rest_meta,
        rest_embs,
        threshold,
        &mut progress_fn,
        &is_cancelled_fn,
    ) {
        Some(c) => c,
        None => return Ok(0),
    };
    drop(rest_meta);

    // Last chance to cancel before anything is written.
    if is_cancelled_fn() {
        return Ok(0);
    }

    // 4. Write the result: new clusters keep matching automatic persons; anchored faces join
    //    their locked person.
    let total_clusters = clusters.len();
    progress_fn(ClusterProgress {
        phase: "assign".to_string(),
        current: 0,
        total: total_clusters,
    });

    let total_assigned = Person::apply_clusters(&clusters, &anchored)?;

    progress_fn(ClusterProgress {
        phase: "assign".to_string(),
        current: total_clusters,
        total: total_clusters,
    });

    // 5. Generate thumbnails
    progress_fn(ClusterProgress {
        phase: "thumbnail".to_string(),
        current: 0,
        total: total_clusters,
    });

    Person::update_all_thumbnails()?;

    progress_fn(ClusterProgress {
        phase: "thumbnail".to_string(),
        current: total_clusters,
        total: total_clusters,
    });

    Ok(total_assigned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: &[f32]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn next_number_skips_existing_names() {
        let names = ["Person 3", "Alice", "Person 12", "Person x"];
        assert_eq!(next_person_number(names.iter().copied()), 13);
        assert_eq!(next_person_number(std::iter::empty()), 1);
    }

    #[test]
    fn match_keeps_person_for_largest_overlap() {
        // Person 1 had faces 1..=6; the new run splits them 4 / 2 and adds an unrelated cluster.
        let old: HashMap<i64, i64> = (1..=6).map(|f| (f, 1)).collect();
        let size: HashMap<i64, usize> = HashMap::from([(1, 6)]);
        let clusters = vec![vec![1, 2, 3, 4], vec![5, 6], vec![100, 101]];
        let m = match_clusters_to_persons(&clusters, &old, &size);
        assert_eq!(m, vec![Some(1), None, None]);
    }

    #[test]
    fn match_rejects_stray_overlap() {
        // One stray face from a big person must not transfer the person to an unrelated cluster.
        let old: HashMap<i64, i64> = (1..=10).map(|f| (f, 1)).collect();
        let size: HashMap<i64, usize> = HashMap::from([(1, 10)]);
        let clusters = vec![vec![1, 50, 51, 52]]; // overlap 1, smaller side 4 -> 1*2 < 4
        let m = match_clusters_to_persons(&clusters, &old, &size);
        assert_eq!(m, vec![None]);
    }

    #[test]
    fn match_is_one_to_one_and_deterministic() {
        let old: HashMap<i64, i64> = [(1, 7), (2, 7), (3, 8), (4, 8)].into_iter().collect();
        let size: HashMap<i64, usize> = HashMap::from([(7, 2), (8, 2)]);
        let clusters = vec![vec![1, 2], vec![3, 4]];
        let m = match_clusters_to_persons(&clusters, &old, &size);
        assert_eq!(m, vec![Some(7), Some(8)]);
    }

    #[test]
    fn attach_assigns_within_threshold_only() {
        let anchors = AnchorSet {
            members: vec![(1, 10)],
            samples: vec![(1, unit(&[1.0, 0.0]))],
        };
        let free = vec![
            FreeFace { file_id: 20, emb: Some(unit(&[1.0, 0.1])) }, // close
            FreeFace { file_id: 21, emb: Some(unit(&[0.0, 1.0])) }, // far
            FreeFace { file_id: 22, emb: None },                    // no embedding
        ];
        let r = attach_to_anchors(&free, &anchors, 0.35, &never).unwrap();
        assert_eq!(r, vec![Some(1), None, None]);
    }

    #[test]
    fn attach_never_puts_same_person_twice_in_one_photo() {
        // Person 1 already has a face in file 10, so a similar face in file 10 is someone else.
        let anchors = AnchorSet {
            members: vec![(1, 10)],
            samples: vec![(1, unit(&[1.0, 0.0]))],
        };
        let free = vec![FreeFace { file_id: 10, emb: Some(unit(&[1.0, 0.05])) }];
        let r = attach_to_anchors(&free, &anchors, 0.35, &never).unwrap();
        assert_eq!(r, vec![None]);

        // Two similar free faces in the same new photo: only the closer one attaches.
        let anchors = AnchorSet {
            members: vec![(1, 10)],
            samples: vec![(1, unit(&[1.0, 0.0]))],
        };
        let free = vec![
            FreeFace { file_id: 30, emb: Some(unit(&[1.0, 0.2])) },
            FreeFace { file_id: 30, emb: Some(unit(&[1.0, 0.05])) },
        ];
        let r = attach_to_anchors(&free, &anchors, 0.35, &never).unwrap();
        assert_eq!(r, vec![None, Some(1)]);
    }

    #[test]
    fn attach_picks_nearest_person() {
        let anchors = AnchorSet {
            members: vec![(1, 10), (2, 11)],
            samples: vec![(1, unit(&[1.0, 0.0])), (2, unit(&[0.8, 0.6]))],
        };
        let free = vec![FreeFace { file_id: 40, emb: Some(unit(&[0.9, 0.5])) }];
        let r = attach_to_anchors(&free, &anchors, 0.9, &never).unwrap();
        assert_eq!(r, vec![Some(2)]);
    }

    #[test]
    fn stride_sample_is_bounded_and_even() {
        let items: Vec<Vec<f32>> = (0..1000).map(|i| vec![i as f32]).collect();
        let s = stride_sample(items, 200);
        assert_eq!(s.len(), 200);
        assert!(stride_sample(vec![vec![1.0]], 200).len() == 1);
    }

    #[test]
    fn cancellation_returns_none() {
        let anchors = AnchorSet {
            members: vec![],
            samples: vec![(1, unit(&[1.0, 0.0]))],
        };
        let free = vec![FreeFace { file_id: 1, emb: Some(unit(&[1.0, 0.0])) }];
        assert!(attach_to_anchors(&free, &anchors, 0.5, &|| true).is_none());
    }
}
