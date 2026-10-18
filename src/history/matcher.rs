//! Finds every stretch of a source fingerprint that reappears in an export fingerprint, at any
//! number of different offsets (chromaprint's own matcher only reports the single best offset).

/// Items whose top bits match vote for an alignment offset.
const KEY_BITS: u32 = 12;
/// Offsets need this many votes before their diagonal is examined.
const MIN_VOTES: u32 = 12;
/// Diagonals examined per source, best-voted first.
const MAX_CANDIDATES: usize = 96;
/// Moving-average width, in items, for bit errors along a diagonal.
const SMOOTHING_ITEMS: usize = 12;
/// Mean differing bits (of 32) below which items count as matching; unrelated audio sits near 16.
const MATCH_BIT_ERRORS: f32 = 10.5;
/// Shortest run worth reporting, in items (~3 s).
const MIN_RUN_ITEMS: usize = 24;

/// `source[source_start..source_start + len]` reappears at `export[export_start..]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub source_start: usize,
    pub export_start: usize,
    pub len: usize,
    /// Mean differing bits per item along the run.
    pub bit_errors: f32,
}

impl Run {
    pub fn export_end(&self) -> usize {
        self.export_start + self.len
    }

    /// Longer and cleaner runs win when runs claim the same export items.
    pub fn quality(&self) -> f32 {
        self.len as f32 * (16.0 - self.bit_errors).max(0.0)
    }
}

pub fn find_runs(source: &[u32], export: &[u32]) -> Vec<Run> {
    let runs = candidate_offsets(source, export)
        .into_iter()
        .flat_map(|offset| runs_on_diagonal(source, export, offset))
        .map(|run| ((), run))
        .collect();
    best_non_overlapping(runs).into_iter().map(|((), run)| run).collect()
}

/// Keeps the best runs first; each later run shrinks to its longest stretch of export not
/// already claimed, or goes when too little is left. Sorted by export position.
pub fn best_non_overlapping<T>(mut runs: Vec<(T, Run)>) -> Vec<(T, Run)> {
    runs.sort_by(|a, b| b.1.quality().total_cmp(&a.1.quality()));
    let mut kept: Vec<(T, Run)> = Vec::new();
    for (tag, run) in runs {
        let claimed: Vec<(usize, usize)> =
            kept.iter().map(|(_, run)| (run.export_start, run.export_end())).collect();
        if let Some(run) = unclaimed_part(run, &claimed) {
            kept.push((tag, run));
        }
    }
    kept.sort_by_key(|(_, run)| run.export_start);
    kept
}

/// The longest part of `run` outside every claimed export range, if long enough to keep.
fn unclaimed_part(run: Run, claimed: &[(usize, usize)]) -> Option<Run> {
    let mut free = vec![(run.export_start, run.export_end())];
    for &(start, end) in claimed {
        free = free
            .into_iter()
            .flat_map(|(low, high)| [(low, high.min(start)), (low.max(end), high)])
            .filter(|(low, high)| high > low)
            .collect();
    }
    let (low, high) = free.into_iter().max_by_key(|(low, high)| high - low)?;
    (high - low >= MIN_RUN_ITEMS).then(|| Run {
        source_start: run.source_start + (low - run.export_start),
        export_start: low,
        len: high - low,
        bit_errors: run.bit_errors,
    })
}

/// Offsets (`export index - source index`) that many items agree on.
fn candidate_offsets(source: &[u32], export: &[u32]) -> Vec<isize> {
    let buckets = bucket_source(source);
    let shift = source.len();
    let mut votes = vec![0u32; source.len() + export.len()];
    for (export_index, item) in export.iter().enumerate() {
        for &source_index in &buckets[key(*item)] {
            votes[export_index + shift - source_index as usize] += 1;
        }
    }
    let mut peaks: Vec<(u32, usize)> = (0..votes.len())
        .filter(|&i| votes[i] >= MIN_VOTES && is_local_peak(&votes, i))
        .map(|i| (votes[i], i))
        .collect();
    peaks.sort_by_key(|&(votes, _)| std::cmp::Reverse(votes));
    peaks.into_iter().take(MAX_CANDIDATES).map(|(_, i)| i as isize - shift as isize).collect()
}

/// Source positions by key, leaving out keys so common (silence, steady tones) that they vote
/// for every offset at once.
fn bucket_source(source: &[u32]) -> Vec<Vec<u32>> {
    let mut buckets = vec![Vec::new(); 1 << KEY_BITS];
    for (index, item) in source.iter().enumerate() {
        buckets[key(*item)].push(index as u32);
    }
    let common = 16 * (source.len() >> KEY_BITS).max(1);
    for bucket in &mut buckets {
        if bucket.len() > common {
            bucket.clear();
        }
    }
    buckets
}

fn key(item: u32) -> usize {
    (item >> (32 - KEY_BITS)) as usize
}

fn is_local_peak(votes: &[u32], index: usize) -> bool {
    let low = index.saturating_sub(2);
    let high = (index + 3).min(votes.len());
    votes[low..high].iter().all(|&other| other <= votes[index])
        && (low..index).all(|i| votes[i] < votes[index])
}

/// Matching stretches along one offset.
fn runs_on_diagonal(source: &[u32], export: &[u32], offset: isize) -> Vec<Run> {
    let first = (-offset).max(0) as usize;
    let last = (export.len() as isize - offset).min(source.len() as isize).max(0) as usize;
    if last <= first {
        return Vec::new();
    }
    let errors: Vec<f32> = (first..last)
        .map(|i| (source[i] ^ export[(i as isize + offset) as usize]).count_ones() as f32)
        .collect();
    let smoothed = moving_average(&errors, SMOOTHING_ITEMS);
    let mut runs = Vec::new();
    let mut start = None;
    for (position, &error) in smoothed.iter().chain(std::iter::once(&f32::MAX)).enumerate() {
        match (error < MATCH_BIT_ERRORS, start) {
            (true, None) => start = Some(position),
            (false, Some(begin)) => {
                start = None;
                if position - begin < MIN_RUN_ITEMS {
                    continue;
                }
                let source_start = first + begin;
                runs.push(Run {
                    source_start,
                    export_start: (source_start as isize + offset) as usize,
                    len: position - begin,
                    bit_errors: errors[begin..position].iter().sum::<f32>()
                        / (position - begin) as f32,
                });
            }
            _ => {}
        }
    }
    runs
}

fn moving_average(values: &[f32], width: usize) -> Vec<f32> {
    let half = width / 2;
    let mut prefix = Vec::with_capacity(values.len() + 1);
    prefix.push(0.0f64);
    for value in values {
        prefix.push(prefix.last().copied().unwrap_or_default() + f64::from(*value));
    }
    (0..values.len())
        .map(|i| {
            let (low, high) = (i.saturating_sub(half), (i + half + 1).min(values.len()));
            ((prefix[high] - prefix[low]) / (high - low) as f64) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random items standing in for real fingerprints.
    fn noise(seed: u64, len: usize) -> Vec<u32> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (state >> 32) as u32
            })
            .collect()
    }

    #[test]
    fn finds_reordered_and_partial_stretches_of_the_source() {
        let source = noise(1, 2_000);
        let filler = noise(2, 150);
        // Export: source[100..500], unrelated filler, source[900..1300] with three flipped bits per item.
        let mut export: Vec<u32> = source[100..500].to_vec();
        export.extend(&filler);
        export.extend(source[900..1_300].iter().map(|item| item ^ 0b111));

        let runs = find_runs(&source, &export);
        assert_eq!(runs.len(), 2, "{runs:?}");
        let (first, second) = (runs[0], runs[1]);
        assert!(
            first.source_start.abs_diff(100) <= 6 && first.export_start.abs_diff(0) <= 6,
            "{first:?}"
        );
        assert!(first.len.abs_diff(400) <= 12, "{first:?}");
        assert!(
            second.source_start.abs_diff(900) <= 6 && second.export_start.abs_diff(550) <= 6,
            "{second:?}"
        );
        assert!((second.bit_errors - 3.0).abs() < 1.0, "{second:?}");
    }

    #[test]
    fn unrelated_audio_produces_no_runs() {
        assert!(find_runs(&noise(3, 3_000), &noise(4, 3_000)).is_empty());
    }
}
