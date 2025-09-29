#![allow(dead_code)]

/// ClosedIntervalSet，多次mark可能會耗時，不過一次更新事件不會有很多mark，在目前使用場景下沒有效能瓶頸
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyLines {
    // 不重疊、已排序的閉區間 [start, end]
    ranges: Vec<(usize, usize)>,
}

impl DirtyLines {
    /// O(1)
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// O(N) - 依賴於 `mark_inclusive_range`
    pub fn mark<I: MarkIndex>(&mut self, index: I) {
        let (start, end) = index.to_range();

        self.mark_inclusive_range(start, end);
    }

    /// O(N) - 在最壞情況下 (合併所有區間)，需要遍歷並複製所有區間。
    /// 在平均情況下，如果只有常數個區間被合併，複雜度接近 O(log N) + O(K) ≈ O(N)
    ///
    /// 具體來說，由於使用了線性遍歷來尋找合併點，然後進行了區間向量的重建，
    /// 雖然可以優化為 O(log N + K)，但目前的實現是 O(N) (因為需要複製未被影響的區間)。
    fn mark_inclusive_range(&mut self, start: usize, end: usize) {
        if start > end {
            return;
        }

        let search_start = start.saturating_sub(1);

        // 找第一個 e >= search_start
        let i_start = self.ranges.partition_point(|&(_, e)| e < search_start);

        // --- 合併範圍 ---
        let mut new_start = start;
        let mut new_end = end;
        let mut i_end = i_start;
        let search_end_boundary = end.saturating_add(1);

        while i_end < self.ranges.len() {
            let (s, e) = self.ranges[i_end];
            if s > search_end_boundary {
                break;
            }
            new_start = std::cmp::min(new_start, s);
            new_end = std::cmp::max(new_end, e);
            i_end += 1;
        }

        let num_to_remove = i_end - i_start;

        if num_to_remove > 0 {
            self.ranges[i_start] = (new_start, new_end);
            self.ranges.drain((i_start + 1)..i_end);
        } else {
            self.ranges.insert(i_start, (new_start, new_end));
        }
    }

    /// O(log N) - 使用二分查找 (binary_search_by)
    pub fn is_marked(&self, line: usize) -> bool {
        self.ranges
            .binary_search_by(|(start, end)| {
                if line < *start {
                    std::cmp::Ordering::Greater
                } else if line > *end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    /// O(1)
    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    /// O(1) - 僅初始化迭代器
    pub fn iter_range<I: MarkIndex>(&self, index: I) -> DirtyRangeIter<'_> {
        let (start, end_inclusive) = index.to_range();

        // 將閉區間 (start, end_inclusive) 轉換為半開區間 (start, end_exclusive)
        let end_exclusive = end_inclusive.saturating_add(1);

        DirtyRangeIter {
            dirty_lines: self,
            current: start,
            end: end_exclusive, // 使用 half-open end
        }
    }

    /// O(log N) - 執行二分查找來定位起始點
    pub fn iter_dirty_ranges<I: MarkIndex>(&self, index: I) -> DirtyRangesIter<'_> {
        let (query_start, query_end_inclusive) = index.to_range();
        let query_end_exclusive = query_end_inclusive.saturating_add(1);

        let start_index = self
            .ranges
            .binary_search_by(|&(_, e)| e.cmp(&query_start))
            .unwrap_or_else(|i| i);

        let mut actual_start_index = start_index;
        while actual_start_index < self.ranges.len()
            && self.ranges[actual_start_index].1 < query_start
        {
            actual_start_index += 1;
        }

        DirtyRangesIter {
            ranges: self.ranges[actual_start_index..].iter(),
            query_start,
            query_end: query_end_exclusive,
        }
    }
}

pub struct DirtyRangeIter<'a> {
    dirty_lines: &'a DirtyLines,
    current: usize,
    end: usize,
}
pub struct DirtyRangesIter<'a> {
    ranges: std::slice::Iter<'a, (usize, usize)>,
    query_start: usize,
    query_end: usize,
}

impl<'a> Iterator for DirtyRangeIter<'a> {
    type Item = (usize, bool);
    /// O(log N) - 每迭代一次，調用一次 O(log N) 的 `is_marked`。
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let line = self.current;
            self.current += 1;
            // 複雜度來自這裡：O(log N)
            let is_dirty = self.dirty_lines.is_marked(line);
            Some((line, is_dirty))
        } else {
            None
        }
    }
}

impl<'a> Iterator for DirtyRangesIter<'a> {
    type Item = (usize, usize);

    /// O(1) amortized / O(K) total - 在整個迭代過程中，每個區間最多被檢查一次。
    /// K 是與查詢範圍重疊或位於查詢範圍內的 dirty 區間數量。
    fn next(&mut self) -> Option<Self::Item> {
        let query_start = self.query_start;
        let query_end = self.query_end;

        // O(1) amortized
        for &(s, e) in self.ranges.by_ref() {
            // 如果當前 dirty 區間的起始點 s 已經超過查詢範圍的結束點 query_end，則結束
            if s >= query_end {
                return None;
            }

            // 計算交集 [intersection_start, intersection_end] (閉區間)
            let intersection_start = s.max(query_start);
            let intersection_end = e.min(query_end.saturating_sub(1));

            // 檢查交集是否有效 (start <= end)
            if intersection_start <= intersection_end {
                return Some((intersection_start, intersection_end));
            }
            // 如果交集無效，表示 dirty 範圍 e 在 query_start 之前 (由於我們已經用 binary search 跳過了，這應該很少發生，除非查詢範圍為空或 dirty 範圍極短且恰好位於邊界)。
        }
        None
    }
}

pub trait MarkIndex {
    fn to_range(&self) -> (usize, usize);
}

impl MarkIndex for usize {
    fn to_range(&self) -> (usize, usize) {
        (*self, *self) // 單行，閉區間 [N, N]
    }
}

impl MarkIndex for std::ops::Range<usize> {
    fn to_range(&self) -> (usize, usize) {
        // [start, end) -> [start, end - 1]
        if self.is_empty() {
            (self.start, self.start)
        } else {
            (self.start, self.end - 1)
        }
    }
}

impl MarkIndex for std::ops::RangeInclusive<usize> {
    fn to_range(&self) -> (usize, usize) {
        // [start, end] -> [start, end]
        if self.is_empty() {
            (*self.start(), *self.start())
        } else {
            (*self.start(), *self.end())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_single_line() {
        let mut dl = DirtyLines::new();
        // 標記單行
        dl.mark(5);
        assert_eq!(*dl.ranges, vec![(5, 5)]);

        dl.mark(10);
        assert_eq!(*dl.ranges, vec![(5, 5), (10, 10)]);

        // 標記相鄰的行
        dl.mark(6);
        assert_eq!(*dl.ranges, vec![(5, 6), (10, 10)]);
    }

    #[test]
    fn test_mark_range_half_open() {
        let mut dl = DirtyLines::new();

        // 標記半開區間 (Range) [5, 8) -> [5, 7]
        dl.mark(5..8);
        assert_eq!(*dl.ranges, vec![(5, 7)]);

        // 標記空範圍 (Range)
        dl.mark(10..10);
        assert_eq!(*dl.ranges, vec![(5, 7), (10, 10)]);
    }

    #[test]
    fn test_mark_range_inclusive() {
        let mut dl = DirtyLines::new();

        // 標記閉區間 (RangeInclusive) [5, 8]
        dl.mark(5..=8);
        assert_eq!(*dl.ranges, vec![(5, 8)]);

        #[allow(clippy::reversed_empty_ranges)]
        // 標記空範圍 (RangeInclusive) [10, 9] -> [10, 10]
        dl.mark(10..=9);
        assert_eq!(*dl.ranges, vec![(5, 8), (10, 10)]); // 5..=8 合併了 10..=9 成為 5..=10
    }

    #[test]
    fn test_merging_complex() {
        let mut dl = DirtyLines::new();
        dl.mark(10..15); // [10, 14]
        dl.mark(30..35); // [30, 34]
        dl.mark(50); // [50, 50]
        assert_eq!(*dl.ranges, vec![(10, 14), (30, 34), (50, 50)]);

        // 跨越合併 (使用閉區間) [13, 31]
        dl.mark(13..=31);
        assert_eq!(*dl.ranges, vec![(10, 34), (50, 50)]);

        // 相鄰合併 (使用半開區間) [34, 36) -> [34, 35]
        dl.mark(34..36);
        assert_eq!(*dl.ranges, vec![(10, 35), (50, 50)]);

        // 跨越所有範圍
        dl.mark(5..55); // [5, 54]
        assert_eq!(*dl.ranges, vec![(5, 54)]);
    }

    #[test]
    fn test_is_dirty_accuracy() {
        let mut dl = DirtyLines::new();
        dl.mark(10..=19); // [10, 19]
        dl.mark(30); // [30, 30]

        // 範圍之內
        assert!(dl.is_marked(10));
        assert!(dl.is_marked(19));
        assert!(dl.is_marked(30));

        // 範圍之外
        assert!(!dl.is_marked(9));
        assert!(!dl.is_marked(20));
        assert!(!dl.is_marked(31));
    }

    #[test]
    fn test_iter_with_inclusive_query() {
        let mut dl = DirtyLines::new();
        dl.mark(5..8); // [5, 7]
        dl.mark(10); // [10, 10]

        // 查詢閉區間 4..=8: 包含行 4, 5, 6, 7, 8
        let results_inclusive: Vec<(usize, bool)> = dl.iter_range(4..=8).collect();

        assert_eq!(
            results_inclusive,
            vec![
                (4, false),
                (5, true),
                (6, true),
                (7, true),
                (8, false), // 8 不在 [5, 7] 內
            ]
        );
    }

    #[test]
    fn test_dirty_ranges_iter_with_inclusive_query() {
        let mut dl = DirtyLines::new();
        dl.mark(5..10); // [5, 9]
        dl.mark(15..20); // [15, 19]

        // 查詢閉區間 8..=16: 包含行 8 到 16
        let query_inclusive = 8..=16;
        let intersecting_ranges: Vec<(usize, usize)> =
            dl.iter_dirty_ranges(query_inclusive).collect();

        // 預期交集：
        // 1. [5, 9] & [8, 16] -> [8, 9]
        // 2. [15, 19] & [8, 16] -> [15, 16]
        assert_eq!(intersecting_ranges, vec![(8, 9), (15, 16)]);
    }
}
