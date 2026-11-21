//! Offset pagination with a hard maximum page size (backend.md §2).

/// Largest page any list endpoint returns, whatever the client asks for.
pub const MAX_PAGE_SIZE: u32 = 100;
pub const DEFAULT_PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: i64,
    pub offset: i64,
}

impl PageRequest {
    /// Clamps what the client asked for: missing or zero limit means the default.
    pub fn new(limit: Option<u32>, offset: Option<u32>) -> Self {
        let limit = match limit {
            None | Some(0) => DEFAULT_PAGE_SIZE,
            Some(n) => n.min(MAX_PAGE_SIZE),
        };
        Self {
            limit: i64::from(limit),
            offset: i64::from(offset.unwrap_or(0)),
        }
    }
}

/// One page of results and how many there are in total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_clamped() {
        assert_eq!(PageRequest::new(None, None).limit, 50);
        assert_eq!(PageRequest::new(Some(0), None).limit, 50);
        assert_eq!(
            PageRequest::new(Some(10), Some(20)),
            PageRequest {
                limit: 10,
                offset: 20
            }
        );
        assert_eq!(PageRequest::new(Some(10_000), None).limit, 100);
    }
}
